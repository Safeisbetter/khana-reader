
pub fn stretch(input: &[f32], speed: f64, rate: usize) -> Vec<f32> {
    if (speed - 1.0).abs() < 0.03 || input.len() < rate / 10 {
        return input.to_vec();
    }
    let min_p = rate / 400;
    let max_p = rate / 65;
    let max_required = 2 * max_p;
    let mut out: Vec<f32> = Vec::with_capacity((input.len() as f64 / speed) as usize + 4096);
    let mut position = 0usize;
    let mut remaining = 0usize;
    let mut prev_period = 0usize;
    while position + max_required <= input.len() {
        if remaining > 0 {
            let n = max_required.min(remaining);
            out.extend_from_slice(&input[position..position + n]);
            remaining -= n;
            position += n;
            continue;
        }
        let period = find_period(input, position, min_p, max_p, prev_period);
        prev_period = period;
        if speed > 1.0 {
            let new_samples = if speed >= 2.0 {
                (period as f64 / (speed - 1.0)) as usize
            } else {
                remaining = (period as f64 * (2.0 - speed) / (speed - 1.0)) as usize;
                period
            };
            overlap_add(&mut out, new_samples, input, position, input, position + period);
            position += period + new_samples;
        } else {
            let new_samples = if speed < 0.5 {
                (period as f64 * speed / (1.0 - speed)) as usize
            } else {
                remaining = (period as f64 * (2.0 * speed - 1.0) / (1.0 - speed)) as usize;
                period
            };
            out.extend_from_slice(&input[position..position + period]);
            overlap_add(&mut out, new_samples, input, position + period, input, position);
            position += new_samples;
        }
    }
    let tail = &input[position.min(input.len())..];
    if !tail.is_empty() {
        let n = ((tail.len() as f64) / speed).round().max(1.0) as usize;
        for i in 0..n {
            let x = i as f64 * (tail.len() - 1) as f64 / (n.max(2) - 1) as f64;
            let a = x.floor() as usize;
            let b = (a + 1).min(tail.len() - 1);
            let f = (x - a as f64) as f32;
            out.push(tail[a] * (1.0 - f) + tail[b] * f);
        }
    }
    out
}

fn overlap_add(out: &mut Vec<f32>, n: usize, down: &[f32], down_pos: usize, up: &[f32], up_pos: usize) {
    for t in 0..n {
        let (d, u) = (down.get(down_pos + t).copied().unwrap_or(0.0), up.get(up_pos + t).copied().unwrap_or(0.0));
        out.push((d * (n - t) as f32 + u * t as f32) / n as f32);
    }
}

fn find_period(x: &[f32], pos: usize, min_p: usize, max_p: usize, prev: usize) -> usize {
    let skip = 6;
    let mut best = min_p;
    let mut best_diff = f64::MAX;
    let mut p = min_p;
    while p <= max_p {
        let mut d = 0.0f64;
        let mut i = 0;
        while i < max_p {
            d += (x[pos + i] - x[pos + i + p]).abs() as f64;
            i += skip;
        }
        if d < best_diff {
            best_diff = d;
            best = p;
        }
        p += skip;
    }
    let lo = min_p.max(best.saturating_sub(skip));
    let hi = max_p.min(best + skip);
    let mut exact = best;
    best_diff = f64::MAX;
    for q in lo..=hi {
        let mut d = 0.0f64;
        for i in 0..q {
            d += (x[pos + i] - x[pos + i + q]).abs() as f64;
        }
        d /= q as f64;
        if prev > 0 && (q as isize - prev as isize).abs() <= 2 {
            d *= 0.97;
        }
        if d < best_diff {
            best_diff = d;
            exact = q;
        }
    }
    exact
}
