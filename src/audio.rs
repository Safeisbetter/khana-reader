
use std::path::Path;

pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut r = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f64;
            r.samples::<i32>().map(|s| (s.unwrap_or(0) as f64 / scale) as f32).collect()
        }
    };
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
    let n = (rate as f64 * 0.04) as usize;
    let hop = (rate as f64 * 0.01) as usize;
    if x.len() < n * 10 {
        return None;
    }
    let mut planner = realfft::RealFftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(2 * n);
    let inv = planner.plan_fft_inverse(2 * n);
    let lo = rate / 400;
    let hi = rate / 70;
    let mut pitches = Vec::new();
    let mut frames = 0;
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
        fwd.process(&mut buf, &mut spec).ok()?;
        for c in spec.iter_mut() {
            *c = realfft::num_complex::Complex::new(c.norm_sqr(), 0.0);
        }
        inv.process(&mut spec, &mut ac).ok()?;
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
    if frames < 8 || pitches.len() < 8 {
        return None;
    }
    Some(median(&mut pitches))
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

