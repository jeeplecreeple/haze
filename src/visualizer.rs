//! The audio visualizer's data: the player hands every decoded packet to `feed`, and the UI asks
//! `spectrum` for band levels while the Now Playing page is open.

use std::sync::Mutex;

/// librespot decodes 44.1 kHz stereo.
const RATE: f32 = 44_100.0;
const FFT: usize = 2048;
/// About 1.5 s of mono audio, enough to look back past what rodio still has queued.
const RING: usize = 65_536;

struct Ring {
    samples: Vec<f32>,
    /// Mono samples written so far.
    written: usize,
}

static RING_BUF: Mutex<Ring> = Mutex::new(Ring { samples: Vec::new(), written: 0 });

/// Stores interleaved stereo samples (as librespot hands them to the audio output) as mono.
pub fn feed(interleaved: &[f64]) {
    let Ok(mut ring) = RING_BUF.lock() else { return };
    if ring.samples.is_empty() {
        ring.samples = vec![0.0; RING];
    }
    for pair in interleaved.chunks_exact(2) {
        let i = ring.written % RING;
        ring.samples[i] = ((pair[0] + pair[1]) * 0.5) as f32;
        ring.written += 1;
    }
}

/// Levels from 0 to 1 for `bands` frequency bands (low to high, spaced like hearing is), for the
/// audio being heard right now: `delay_ms` is how far the output runs behind what was decoded.
pub fn spectrum(bands: usize, delay_ms: u32) -> Vec<f32> {
    let mut window = vec![0.0f32; FFT];
    {
        let Ok(ring) = RING_BUF.lock() else { return vec![0.0; bands] };
        let delay = (delay_ms as f32 / 1000.0 * RATE) as usize;
        if ring.samples.is_empty() || ring.written < FFT + delay {
            return vec![0.0; bands];
        }
        let end = ring.written - delay;
        for (k, w) in window.iter_mut().enumerate() {
            *w = ring.samples[(end - FFT + k) % RING];
        }
    }
    levels(&window, bands)
}

fn levels(window: &[f32], bands: usize) -> Vec<f32> {
    let n = window.len();
    let mut re: Vec<f32> = window
        .iter()
        .enumerate()
        .map(|(k, s)| s * (0.5 - 0.5 * (std::f32::consts::TAU * k as f32 / n as f32).cos()))
        .collect();
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    let mag: Vec<f32> = (0..n / 2).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt() / n as f32).collect();

    let (lo, hi) = (40.0f32, 16_000.0f32);
    let bin = |f: f32| ((f / RATE * n as f32) as usize).clamp(1, n / 2 - 1);
    (0..bands.max(1))
        .map(|b| {
            let f0 = lo * (hi / lo).powf(b as f32 / bands as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / bands as f32);
            let (a, z) = (bin(f0), bin(f1).max(bin(f0) + 1));
            let peak = mag[a..z].iter().copied().fold(0.0, f32::max);
            // Highs carry less energy than lows; tilt them up so the whole ring moves.
            let tilt = 1.0 + 2.5 * b as f32 / bands as f32;
            let db = 20.0 * (peak * tilt).max(1e-6).log10();
            ((db + 62.0) / 50.0).clamp(0.0, 1.0)
        })
        .collect()
}

/// In-place radix-2 FFT; `re.len()` must be a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -std::f32::consts::TAU / len as f32;
        let (wr, wi) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                (cr, ci) = (cr * wr - ci * wi, cr * wi + ci * wr);
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lights_its_band() {
        let tone: Vec<f32> = (0..FFT).map(|k| (std::f32::consts::TAU * 1000.0 * k as f32 / RATE).sin() * 0.5).collect();
        let bands = levels(&tone, 24);
        let loudest = bands.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        // 1 kHz sits a little past the middle of a 40 Hz to 16 kHz log scale.
        assert!((11..=14).contains(&loudest), "loudest band {loudest}: {bands:?}");
        assert!(bands[0] < 0.2);
    }

    #[test]
    fn silence_is_flat() {
        assert!(levels(&vec![0.0; FFT], 16).iter().all(|&l| l == 0.0));
    }
}
