macro_rules! fn_ok {
    ($call:ident $(, ($arg:ident : $t:ty))* $(,)?) => {
			#[allow(unused_variables)]
			pub unsafe fn $call ($($arg : $t, )*) -> ::std::os::raw::c_int {
				print!(
					concat!("[{}] ",  stringify!($call), " "),
					API_START_TIME.elapsed().as_secs_f32()
				);
			  $(
					print!(
						concat!("(", stringify!($arg), " = {:?})"),
						$arg
					);
				)*
				println!("");
				APIError::RP_OK as std::os::raw::c_int
			}
		}
}

use std::f32::consts::PI;
use std::time::Instant;

use lazy_static::lazy_static;

lazy_static! {
    static ref API_START_TIME: Instant = Instant::now();
}

pub fn core_mock_init() {
    lazy_static::initialize(&API_START_TIME);
    println!(
        "[{}]: Initializing RP API",
        API_START_TIME.elapsed().as_secs_f32()
    );
}

fn_ok!(rp_InitReset, (reset: bool));
fn_ok!(rp_Release);

pub type rp_channel_t = ::std::os::raw::c_uint;
pub type rp_waveform_t = ::std::os::raw::c_uint;
pub type rp_gen_mode_t = ::std::os::raw::c_uint;
pub type rp_trig_src_t = ::std::os::raw::c_uint;

fn_ok!(rp_GenOutEnable, (core_ch: rp_channel_t));
fn_ok!(rp_GenOutDisable, (core_ch: rp_channel_t));
fn_ok!(rp_GenAmp, (core_ch: rp_channel_t), (amplitude: f32));
fn_ok!(rp_GenOffset, (core_ch: rp_channel_t), (offset: f32));
fn_ok!(rp_GenFreq, (core_ch: rp_channel_t), (frequency: f32));
fn_ok!(
    rp_GenWaveform,
    (core_ch: rp_channel_t),
    (type_: rp_waveform_t)
);
fn_ok!(
    rp_GenArbWaveform,
    (core_ch: rp_channel_t),
    (waveform: *mut f32),
    (length: u32)
);
fn_ok!(rp_GenMode, (core_ch: rp_channel_t), (mode: rp_gen_mode_t));
fn_ok!(
    rp_GenBurstCount,
    (core_ch: rp_channel_t),
    (num: ::std::os::raw::c_int)
);
fn_ok!(
    rp_GenBurstLastValue,
    (core_ch: rp_channel_t),
    (amplitude: f32)
);
fn_ok!(
    rp_GenTriggerSource,
    (core_ch: rp_channel_t),
    (src: rp_trig_src_t)
);

pub type rp_dpin_t = ::std::os::raw::c_uint;
pub type rp_pinState_t = ::std::os::raw::c_uint;
pub type rp_pinDirection_t = ::std::os::raw::c_uint;

fn_ok!(
    rp_DpinSetDirection,
    (pin: rp_dpin_t),
    (dir: rp_pinDirection_t)
);
fn_ok!(rp_DpinSetState, (pin: rp_dpin_t), (state: rp_pinState_t));
pub unsafe fn rp_DpinGetState(pin: rp_dpin_t, state: *mut rp_pinState_t) -> ::std::os::raw::c_int {
    *state = 0;
    println!(
        "[{}] rp_DpinGetState",
        API_START_TIME.elapsed().as_secs_f32()
    );
    APIError::RP_OK as ::std::os::raw::c_int
}

pub type rp_acq_trig_src_t = ::std::os::raw::c_uint;
pub type rp_acq_trig_state_t = ::std::os::raw::c_uint;

static mut BUFF_A: [u32; 16384] = [0; 16384];
static mut BUFF_B: [u32; 16384] = [0; 16384];
pub const ADC_SAMPLE_RATE: f64 = 125000000.0;

fn_ok!(rp_AcqSetTriggerSrc, (src: rp_acq_trig_src_t));
fn_ok!(rp_AcqSetDecimationFactor, (decimation: u32));
fn_ok!(rp_AcqSetTriggerDelay, (decimated_data_num: i32));
fn_ok!(rp_AcqStart);
fn_ok!(rp_AcqStop);

pub unsafe fn rp_AcqGetTriggerState(state: *mut rp_acq_trig_state_t) -> ::std::os::raw::c_int {
    *state = 0;
    println!(
        "[{}] rp_AcqGetTriggerState",
        API_START_TIME.elapsed().as_secs_f32()
    );
    APIError::RP_OK as ::std::os::raw::c_int
}
pub unsafe fn rp_AcqGetWritePointerAtTrig(pos: *mut u32) -> ::std::os::raw::c_int {
    *pos = 0;
    println!(
        "[{}] rp_AcqGetWritePointerAtTrig",
        API_START_TIME.elapsed().as_secs_f32()
    );
    APIError::RP_OK as ::std::os::raw::c_int
}

pub unsafe fn rp_jmd_AcqGetRawBuffer(channel: rp_channel_t) -> *const u32 {
    match channel {
        0 => {
            let freq = 3470. * 2.0 * PI / (16384.0 * 1550.0);
            for i in 0..16384 {
                BUFF_A[i] = ((freq * i as f32 - 1.3).cos() * 100.0 + 1000.0) as u32;
            }
            &BUFF_A as *const u32
        }
        1 => {
            let freq = 3470. * 2.0 * PI / (16384.0 * 1114.0);
            for i in 0..16384 {
                BUFF_B[i] = (120.0 * (freq * i as f32 - 2.1).cos() + 800.0) as u32;
            }
            &BUFF_B as *const u32
        }
        _ => {
            panic!("illegal channel");
        }
    }
}
