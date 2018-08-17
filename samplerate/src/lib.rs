//! High-level bindings for [libsamplerate](http://www.mega-nerd.com/libsamplerate/)
//! audio sample rate converter.
//!
//! Quickstart:
//!   * Use [``convert``](fn.convert.html) to process a single batch of samples.
//!   * Use [``Converter``](struct.Converter.html) to process a continuous stream of samples.

// It's impossible to usefully expose the callback-based libsamplerate API because it captures
// a pointer provided by the callback indefinitely, effectively leaking the buffer until the end
// of the converter lifetime.

#![no_std]

#[cfg(any(test, doctest))]
extern crate std;
extern crate libc;
extern crate samplerate_sys;

use core::{slice, str, fmt};

use libc::{c_int, c_long, strlen};
use samplerate_sys::*;

/// Interpolator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Interpolator {
    SincBestQuality = SRC_SINC_BEST_QUALITY,
    SincMediumQuality = SRC_SINC_MEDIUM_QUALITY,
    SincFastest = SRC_SINC_FASTEST,
    ZeroOrderHold = SRC_ZERO_ORDER_HOLD,
    Linear = SRC_LINEAR,
    #[doc(hidden)]
    __Nonexhaustive
}

/// Conversion error.
#[derive(Debug, Eq)]
pub struct Error {
    code: c_int,
    desc: Option<&'static str>
}

impl Error {
    fn from_code(code: c_int) -> Error {
        unsafe {
            let msg = src_strerror(code);
            let desc = if msg.is_null() {
                None
            } else {
                let len = strlen(msg);
                Some(str::from_utf8_unchecked(slice::from_raw_parts(msg as *const u8, len)))
            };
            Error { code, desc }
        }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        self.code == other.code
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.desc {
            Some(desc) => write!(f, "{}", desc),
            None => write!(f, "unknown ({})", self.code)
        }
    }
}

/// Conversion result.
type Result<T> = core::result::Result<T, Error>;

fn make_data(channels: usize, ratio: f64, end: bool,
             input: &[f32], output: &mut [f32]) -> SRC_DATA {
    assert!(input.len() % channels == 0, "input must be an even number of frames");
    assert!(output.len() % channels == 0, "output must be an even number of frames");
    SRC_DATA {
        data_in:            input.as_ptr(),
        data_out:           output.as_mut_ptr(),
        input_frames:       (input.len() / channels) as c_long,
        output_frames:      (output.len() / channels) as c_long,
        input_frames_used:  0,
        output_frames_gen:  0,
        end_of_input:       end as c_int,
        src_ratio:          ratio,
    }
}

/// Perform a single conversion from input buffer to output buffer with a fixed conversion ratio.
///
/// This function should only be used to convert a complete buffer at once; to convert a buffer
/// chunk by chunk, use [``Converter``](struct.Converter.html). Otherwise, artifacts will appear
/// at chunk boundaries.
///
/// Returns the number of used input samples and generated output samples, respectively.
pub fn convert(interpolator: Interpolator, channels: usize, ratio: f64,
               input: &[f32], output: &mut [f32]) -> Result<(usize, usize)> {
    let mut data = make_data(channels, ratio, /*end=*/true, input, output);
    let error = unsafe { src_simple(&mut data as *mut _, interpolator as c_int,
                                    channels as c_int) };
    if error != 0 {
        return Err(Error::from_code(error))
    }
    Ok((data.input_frames_used as usize * channels as usize,
        data.output_frames_gen as usize * channels as usize))
}

/// Interface for performing a continuous conversion from input stream to output stream with
/// a variable, smoothly interpolated conversion ratio.
pub struct Converter {
    state: *mut SRC_STATE
}

impl Converter {
    /// Create a converter.
    pub fn new(interpolator: Interpolator, channels: usize) -> Result<Converter> {
        let mut error: c_int = 0;
        let state = unsafe { src_new(interpolator as c_int, channels as c_int,
                                     &mut error as *mut _) };
        if state.is_null() {
            return Err(Error::from_code(error))
        }
        Ok(Converter { state })
    }

    /// Retrieve the number of channels used by the converter.
    pub fn channels(&self) -> usize {
        unsafe { src_get_channels(self.state) as usize }
    }

    /// Reset the internal state to the same state it had after [``new``](#method.new).
    pub fn reset(&mut self) -> Result<()> {
        let error = unsafe { src_reset(self.state) };
        if error != 0 {
            return Err(Error::from_code(error))
        }
        Ok(())
    }

    /// Set the starting conversion ratio for the next call to [``convert``](#method.convert).
    ///
    /// Calling this function achieves a step response in conversion ratio instead of smooth
    /// interpolation.
    pub fn set_ratio(&mut self, ratio: f64) -> Result<()> {
        let error = unsafe { src_set_ratio(self.state, ratio) };
        if error != 0 {
            return Err(Error::from_code(error))
        }
        Ok(())
    }

    /// Convert samples using internal state, smoothly interpolating ratio.
    ///
    /// The size of both ``input`` and ``output`` must be a multiple of the converter's channel
    /// count. If there is no more input data, provide ``None`` as ``input``, and the converter
    /// will flush its internal state.
    ///
    /// Returns the number of used input samples and generated output samples, respectively.
    /// The sample numbers may be used to partition the input and output arrays.
    pub fn convert(&mut self, ratio: f64, input: Option<&[f32]>, output: &mut [f32])
            -> Result<(usize, usize)> {
        let channels = self.channels();
        let mut data = make_data(channels, ratio, input.is_none(), input.unwrap_or(&[]), output);
        let error = unsafe { src_process(self.state, &mut data as *mut _) };
        if error != 0 {
            return Err(Error::from_code(error))
        }
        Ok((data.input_frames_used as usize * channels,
            data.output_frames_gen as usize * channels))
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        unsafe { src_delete(self.state); }
    }
}

#[cfg(test)]
mod test {
    use std::f32;
    use std::vec::Vec;
    use std::vec;
    use super::*;

    fn make_fixture(size: usize, cos: bool) -> Vec<f32> {
        let step = f32::consts::PI * 2.0 / size as f32;
        let mut data = Vec::new();
        let mut value = 0.0f32;
        for _ in 0..size {
            data.push(value.sin());
            if cos { data.push(value.cos()); }
            value += step;
        }
        data
    }

    fn test_convert_ch(ch2: bool) {
        let input = make_fixture(1000, ch2);
        let expect = make_fixture(2000, ch2);
        let mut output = vec![0.; expect.len()];
        let channels = if ch2 { 2 } else { 1 };
        let (used, gen) = convert(Interpolator::SincBestQuality, channels, 2.0,
                                  &input, &mut output).unwrap();
        assert_eq!(used, input.len());
        assert_eq!(gen, output.len());
        for (o, e) in output.iter().zip(expect.iter())
                .skip(10 * channels)
                .take(output.len() - 20 * channels) {
            assert!((o - e).abs() < 0.05);
        }
    }

    #[test]
    fn test_convert_1ch() {
        test_convert_ch(false)
    }

    #[test]
    fn test_convert_2ch() {
        test_convert_ch(true)
    }

    fn test_push_converter_ch(ch2: bool) {
        let input = make_fixture(1000, ch2);
        let expect = make_fixture(2000, ch2);
        let mut output = vec![0.; expect.len()];
        let ch = if ch2 { 2 } else { 1 };
        let mut conv = Converter::new(Interpolator::SincBestQuality, ch).unwrap();
        assert_eq!(conv.convert(2.0, Some(&input[..500 * ch]), &mut output[..]).unwrap(),
                   (500 * ch, 712  * ch));
        assert_eq!(conv.convert(2.0, Some(&input[500 * ch..]), &mut output[712 * ch..]).unwrap(),
                   (500 * ch, 1000 * ch));
        assert_eq!(conv.convert(2.0, None, &mut output[1712 * ch..]).unwrap(),
                   (0   * ch, 288  * ch));
        for (o, e) in output.iter().zip(expect.iter())
                .skip(10).take(output.len() - 20) {
            assert!((o - e).abs() < 0.05);
        }
    }

    #[test]
    fn test_push_converter_1ch() {
        test_push_converter_ch(false)
    }

    #[test]
    fn test_push_converter_2ch() {
        test_push_converter_ch(true)
    }
}
