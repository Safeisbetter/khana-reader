
use std::path::Path;

pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut raw = crate::vault::read_bytes(path).map_err(|e| e.to_string())?;
    let mut r = hound::WavReader::new(std::io::Cursor::new(&raw)).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f64;
            r.samples::<i32>().map(|s| (s.unwrap_or(0) as f64 / scale) as f32).collect()
        }
    };
    crate::ffi::wipe::bytes(&mut raw);
    let mono = if ch == 1 {
        data
    } else {
        data.chunks(ch).map(|c| (c.iter().map(|x| *x as f64).sum::<f64>() / ch as f64) as f32).collect()
    };
    Ok((mono, spec.sample_rate))
}

pub fn to_pcm16(audio: &[f32]) -> Vec<i16> {
    audio.iter().map(|x| (x.clamp(-1.0, 1.0) * 32767.0) as i16).collect()
}

pub fn wav_bytes(audio: &[f32], rate: u32) -> Vec<u8> {
    let pcm = to_pcm16(audio);
    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    let data_len = (pcm.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..200 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    sum
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        let y = std::f64::consts::PI * x;
        y.sin() / y
    }
}

fn firwin_kaiser(numtaps: usize, cutoff: f64, beta: f64) -> Vec<f64> {
    let alpha = 0.5 * (numtaps as f64 - 1.0);
    let mut h: Vec<f64> = (0..numtaps).map(|i| cutoff * sinc(cutoff * (i as f64 - alpha))).collect();
    let i0b = bessel_i0(beta);
    for (n, v) in h.iter_mut().enumerate() {
        let r = 2.0 * n as f64 / (numtaps as f64 - 1.0) - 1.0;
        *v *= bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0b;
    }
    let s: f64 = h.iter().sum();
    h.iter().map(|v| v / s).collect()
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

pub fn resample_poly(x: &[f32], up: usize, down: usize) -> Vec<f32> {
    let g = gcd(up, down);
    let (up, down) = (up / g, down / g);
    if up == 1 && down == 1 {
        return x.to_vec();
    }
    let n_in = x.len();
    let n_out = n_in * up / down + usize::from(n_in * up % down != 0);
    let max_rate = up.max(down);
    let half_len = 10 * max_rate;
    let mut h = firwin_kaiser(2 * half_len + 1, 1.0 / max_rate as f64, 5.0);
    for v in h.iter_mut() {
        *v *= up as f64;
    }
    let n_pre_pad = down - half_len % down;
    let n_pre_remove = (half_len + n_pre_pad) / down;
    let out_len = |len_h: usize| ((n_in.max(1) - 1) * up + len_h - 1) / down + 1;
    let mut n_post_pad = 0;
    while out_len(h.len() + n_pre_pad + n_post_pad) < n_out + n_pre_remove {
        n_post_pad += 1;
    }
    let mut hp = vec![0.0f64; n_pre_pad];
    hp.extend_from_slice(&h);
    hp.extend(std::iter::repeat(0.0).take(n_post_pad));
    let mut y = Vec::with_capacity(n_out);
    for k in n_pre_remove..n_pre_remove + n_out {
        let n = k * down;
        let mut acc = 0.0f64;
        let lo = if n + 1 > hp.len() { (n + 1 - hp.len()).div_ceil(up) } else { 0 };
        let hi = (n / up).min(n_in.saturating_sub(1));
        if n_in > 0 && lo <= hi {
            for i in lo..=hi {
                acc += x[i] as f64 * hp[n - i * up];
            }
        }
        y.push(acc as f32);
    }
    y
}

pub fn voice_pitch(x: &[f32], rate: usize) -> Option<f64> {
    voice_quality(x, rate).0
}

/// Returns `(median_pitch_hz, voiced_frame_fraction)` over active frames.
/// Real speech has clear harmonic periodicity (voiced_fraction >= 0.16); static/noise/hiss has near-zero.
pub fn voice_quality(x: &[f32], rate: usize) -> (Option<f64>, f64) {
    let n = (rate as f64 * 0.04) as usize;
    let hop = (rate as f64 * 0.01) as usize;
    if x.len() < n * 3 || n == 0 || hop == 0 {
        return (None, 0.0);
    }
    let mut planner = realfft::RealFftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(2 * n);
    let inv = planner.plan_fft_inverse(2 * n);
    let lo = rate / 400;
    let hi = rate / 70;
    let mut pitches = Vec::new();
    let mut frames = 0usize;
    let mut start = 0;
    let mut buf = fwd.make_input_vec();
    let mut spec = fwd.make_output_vec();
    let mut ac = inv.make_output_vec();
    while start < x.len() - n {
        let fr = &x[start..start + n];
        start += hop;
        let mean = fr.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
        let energy: f64 = fr.iter().map(|v| (*v as f64 - mean).powi(2)).sum();
        if energy <= 1e-4 {
            continue;
        }
        frames += 1;
        for (i, b) in buf.iter_mut().enumerate() {
            *b = if i < n { fr[i] as f64 - mean } else { 0.0 };
        }
        if fwd.process(&mut buf, &mut spec).is_err() {
            return (None, 0.0);
        }
        for c in spec.iter_mut() {
            *c = realfft::num_complex::Complex::new(c.norm_sqr(), 0.0);
        }
        if inv.process(&mut spec, &mut ac).is_err() {
            return (None, 0.0);
        }
        let mut best = lo;
        for l in lo..hi {
            if ac[l] > ac[best] {
                best = l;
            }
        }
        if ac[0] != 0.0 && ac[best] / ac[0] > 0.45 {
            pitches.push(rate as f64 / best as f64);
        }
    }
    let voiced_frac = if frames == 0 { 0.0 } else { pitches.len() as f64 / frames as f64 };
    if frames < 2 || pitches.len() < 2 {
        return (None, voiced_frac);
    }
    (Some(median(&mut pitches)), voiced_frac)
}

/// Detects metallic digital buzzing, screeching, static, or codec rail-clipping in synthesized audio (<1 ms).
/// Clean Persian speech has moderate first-difference spectral slope, low ZCR, and near-zero hard clipping;
/// collapsed FlowLM/Mimi digital noise trips one or more of these physical bounds immediately.
pub fn is_digital_noise(x: &[f32], rate: usize) -> bool {
    if x.len() < (rate / 12).max(64) {
        return false;
    }
    let mut sum_sq = 0.0f64;
    let mut diff_sq = 0.0f64;
    let mut zc = 0usize;
    let mut clipped = 0usize;
    let mut active = 0usize;
    let mut prev = x[0];
    for &v in x {
        if !v.is_finite() || v.abs() > 3.5 {
            return true;
        }
        let vf = v as f64;
        sum_sq += vf * vf;
        let d = vf - prev as f64;
        diff_sq += d * d;
        if v.abs() >= 0.02 {
            active += 1;
            if v.abs() >= 0.96 {
                clipped += 1;
            }
            if (v >= 0.0) != (prev >= 0.0) {
                zc += 1;
            }
        }
        prev = v;
    }
    if active < (rate / 25).max(32) || !(sum_sq > 1e-7) {
        return false;
    }
    let clip_frac = clipped as f64 / active as f64;
    if clip_frac > 0.03 {
        return true;
    }
    // First-difference RMS / signal RMS: clean speech ~0.15..0.90; harsh digital hash/screech >1.32
    let diff_ratio = (diff_sq / sum_sq).sqrt();
    if diff_ratio > 1.32 {
        return true;
    }
    // Active-sample zero-crossing rate: speech ~0.04..0.24; broadband noise/metallic buzz >0.38
    let zcr = zc as f64 / active as f64;
    if zcr > 0.38 {
        return true;
    }
    false
}

pub fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

pub fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    (x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64).sqrt()
}

pub fn linspace(a: f32, b: f32, n: usize) -> Vec<f32> {
    if n == 1 {
        return vec![a];
    }
    (0..n).map(|i| (a as f64 + (b as f64 - a as f64) * i as f64 / (n - 1) as f64) as f32).collect()
}

pub fn fade_edges(mut audio: Vec<f32>, rate: usize, ms: usize) -> Vec<f32> {
    let n = (rate * ms / 1000).min(audio.len() / 2);
    if n > 1 {
        let ramp = linspace(0.0, 1.0, n);
        let len = audio.len();
        for i in 0..n {
            audio[i] *= ramp[i];
            audio[len - n + i] *= ramp[n - 1 - i];
        }
    }
    audio
}

/// Active-speech RMS loudness normalizer with smooth soft-knee peak limiting.
/// Ensures all built-in voice models and user recordings ("صدای من") play at the exact same
/// comfortable volume level without ever clipping transients.
pub fn normalize_loudness(mut audio: Vec<f32>, rate: usize, target_rms: f64) -> Vec<f32> {
    if audio.is_empty() || !(target_rms > 0.0) {
        return audio;
    }
    let frame = (rate / 50).max(1); // 20 ms analysis frames
    let mut energies: Vec<f64> = Vec::with_capacity(audio.len() / frame + 1);
    let mut max_e = 0.0f64;
    for chunk in audio.chunks(frame) {
        let e = rms(chunk);
        if e.is_finite() {
            if e > max_e {
                max_e = e;
            }
            energies.push(e);
        }
    }
    if !(max_e > 1e-4) {
        return audio;
    }
    // Measure only active speech frames (ignoring silence/pauses between clauses)
    let floor = (0.15 * max_e).max(0.004);
    let (mut sum_sq, mut count) = (0.0f64, 0usize);
    for e in energies {
        if e >= floor {
            sum_sq += e * e;
            count += 1;
        }
    }
    let active_rms = if count > 0 { (sum_sq / count as f64).sqrt() } else { rms(&audio) };
    if !(active_rms.is_finite() && active_rms > 1e-5) {
        return audio;
    }
    let gain = (target_rms / active_rms).clamp(0.25, 4.5) as f32;
    let knee = 0.88f32;
    let head = 0.08f32;
    for x in audio.iter_mut() {
        let y = *x * gain;
        let ay = y.abs();
        *x = if ay <= knee {
            y
        } else {
            y.signum() * (knee + head * ((ay - knee) / head).tanh())
        };
    }
    audio
}



fn biquad(x: &mut [f32], b: [f64; 3], a: [f64; 3]) {
    let (b0, b1, b2, a1, a2) = (b[0] / a[0], b[1] / a[0], b[2] / a[0], a[1] / a[0], a[2] / a[0]);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for v in x.iter_mut() {
        let x0 = *v as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *v = y0 as f32;
    }
}

fn notch(x: &mut [f32], rate: usize, f: f64, q: f64) {
    let w = 2.0 * std::f64::consts::PI * f / rate as f64;
    let alpha = w.sin() / (2.0 * q);
    biquad(x, [1.0, -2.0 * w.cos(), 1.0], [1.0 + alpha, -2.0 * w.cos(), 1.0 - alpha]);
}

pub fn high_pass(x: &[f32], rate: usize, f: f64) -> Vec<f32> {
    let w = 2.0 * std::f64::consts::PI * f / rate as f64;
    let alpha = w.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let c = w.cos();
    let mut y = x.to_vec();
    biquad(&mut y, [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0], [1.0 + alpha, -2.0 * c, 1.0 - alpha]);
    y
}

fn tone_power(x: &[f32], rate: usize, f: f64) -> f64 {
    let c = 2.0 * (2.0 * std::f64::consts::PI * f / rate as f64).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for v in x {
        let s0 = *v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    (s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0)
}

pub fn tone_level(x: &[f32], rate: usize, f: f64) -> f64 {
    tone_power(x, rate, f)
}

pub fn mains_hum(x: &[f32], rate: usize) -> Option<f64> {
    if x.len() < rate {
        return None;
    }
    [50.0f64, 60.0].into_iter().find(|&f| {
        let around = (tone_power(x, rate, f - 8.0) + tone_power(x, rate, f + 8.0)) / 2.0;
        tone_power(x, rate, f) > 12.0 * around.max(1e-12) && tone_power(x, rate, f) > 1e-7 * x.len() as f64 * x.len() as f64
    })
}

pub fn dehum(x: Vec<f32>, rate: usize) -> Vec<f32> {
    let cut = voice_pitch(&x, rate).map(|p| (0.7 * p).clamp(50.0, 80.0)).unwrap_or(60.0);
    let hum = mains_hum(&x, rate);
    let mut x = high_pass(&high_pass(&x, rate, cut), rate, cut);
    if let Some(f) = hum {
        for k in 1..=3 {
            notch(&mut x, rate, f * k as f64, 16.0);
        }
    }
    x
}

pub fn settle_tail(mut a: Vec<f32>, rate: usize) -> Vec<f32> {
    let f = (rate / 100).max(1);
    if a.len() < 30 * f {
        return a;
    }
    let hp = high_pass(&a, rate, 150.0);
    let env: Vec<f64> = hp.chunks(f).map(rms).collect();
    let mut sorted = env.clone();
    sorted.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
    let level = sorted[sorted.len() * 9 / 10];
    if !(level > 1e-6) {
        return a;
    }
    let Some(last) = env.iter().rposition(|e| *e >= 0.022 * level) else { return a };
    let keep = ((last + 1) * f + rate * 17 / 100).min(a.len());
    if a.len() - keep < 4 * f {
        return a;
    }
    a.truncate(keep);
    let n = (6 * f).min(a.len());
    let start = a.len() - n;
    for i in 0..n {
        let g = 0.5 + 0.5 * (std::f64::consts::PI * i as f64 / n as f64).cos();
        a[start + i] *= g as f32;
    }
    a
}

