// pub use cc1101::{
//   Cc1101, Config, DEVIATN, FSCTRL1, MCSM1, MDMCFG0, MDMCFG1, MDMCFG2, MDMCFG3, MDMCFG4, PKTCTRL1,
// };
// use cc1101::{GdoCfg, PacketLength, FREND0, IOCFG2, PKTCTRL0};
use embedded_hal::spi::SpiDevice;
use heapless::Vec;


use crate::lowlevel::registers::*;
use crate::lowlevel::types::*;
use crate::PacketLength;
use crate::{Cc1101};
// We're gonna look at the registers so we can set our own good settings,
// for variable packet length, no address check, default syncword,
// base frequency 902, channel 0.
//
// 03 RX/TX FIFO Tresholds
// 	Rx attenuation (default is good)
// 	Fifo threshold (default is good)
// 04,05 Sync word (default is good)
// 06 Packet length (default is 0xFF, good)
// 07,08 Packet automation
// 	set preamble quality to 100 (4)
// 	set CRC flush to 1
// 	APPEND_STATUS set to 0
// 	CRC (default is 1 enabled, good)
// 	Packet length config (default is variable 01, good)
// 09 Address (default 0 is good)
// 0A Channel (default 0 is good)
// 0B IF Freq (default is good 395.5kHz for 27MHz clock)
// 	27_000 / (2 ** 10) * 15 (in kHz)
// 0C Freq Offset (deafult 0 is good)
// 0D,0E,0F Freq Carrier
// 	27 / (2 ** 16) * 0x1EC4EC (in MHz)
// 	default is ~831MHz with 27MHz Clock
// 	we want 902.4MHz, which is 0x216C1D
// 10,11,12,13 Modem COnfig
// 	Channel Filter Bandwidth
// 		27_000 / (8 * (4 + 0) * 2 ** 2) (in kHz)
// 		Default is  = ~211kHz
// 		We need ~60kHz which is M=3,E=3
// 	Data Rate
// 		(256 + 34) * 2 ** 12 / 2 ** 28 * 27_000 (in kBaud)
// 		Default is M=34,E=12 = ~119
// 		We can do ~250 which is M=48,E=13
// 		Nvmd, lets leave at 119 so we can turn off DC Filter
// 	DC filter (default on is good)
// 	Modulation (default is good)
// 	Manchester (default off is good)
// 	Forward error correction (default off is good)
// 	Preamble bytes min (default 4 bytes is good)
// 14 Channel Spacing
// 	27_000 / 2 ** 18 * (256 + 248) * 2 ** 2 (in kHz)
// 	Defult is 207.64 at M=248,E=2
// 	We want ~62.5 which would give us 255 channels from 912-928MHz
// 	Closest is 62.416MHz at M=47,E=1
// 15 Deviation
// 	27_000 / 2 ** 17 * (8 + 7) * 2 ** 4 (in kHz)
// 	Default is 49.43 at m=7,e=4
// 	We want ~30, 29.66 is M=1,E=4
// 16,17,18 Main Radio State Machine
// 	Set Autocal to 01
// 19 Freq Offset Compensation (default is good)
// 1A Bit synchronization (default is good)
// 1B,1C,1D AGC Control (default is good)
// 1E,1F Event 0 (default ~1s is good)
// 20 Wake on Radio (default is good)
// 20..2F BS stuff

/// Preamble quality min 16 consecutive changes, CRC, CRC auto flush, no append status, no address check, variable packet length,
/// Address 0, Channel 0, IF 395.5kHz, Carrier 902.4MHz, Filter Bandwidth 60kHz,
/// Data Rate 119kBaud, 4 byte preamble, Mouldation 2-FSK, Channel spacing 62.416MHz, Modulator Deviation 29.66kHz,
/// Autocalibration from Iddle
pub fn config_0<T: SpiDevice>(cc1101: &mut Cc1101<T>) {
    // Set good preamble quality, turn on CRC flush
    // and turn off Append Status on incoming data
    cc1101
        .0
        .write_register(
            Config::PKTCTRL1,
            PKTCTRL1::default()
                .pqt(4)
                .crc_autoflush(1)
                .append_status(0)
                .bits(),
        )
        .unwrap();

    // Set carrier base frequency 902.4 MHz
    cc1101.0.write_register(Config::FREQ2, 0x21).unwrap();
    cc1101.0.write_register(Config::FREQ1, 0x6C).unwrap();
    cc1101.0.write_register(Config::FREQ0, 0x1D).unwrap();

    // Set filter bandwidth to 60 kHz
    cc1101
        .0
        .write_register(
            Config::MDMCFG4,
            MDMCFG4::default().chanbw_m(3).chanbw_e(3).bits(),
        )
        .unwrap();

    // Set channel spacing to 62.416 kHz
    cc1101
        .0
        .write_register(Config::MDMCFG1, MDMCFG1::default().chanspc_e(1).bits())
        .unwrap();
    cc1101
        .0
        .write_register(Config::MDMCFG0, MDMCFG0::default().chanspc_m(47).bits())
        .unwrap();
    // Set deviation to 29.66 kHz
    cc1101
        .0
        .write_register(
            Config::DEVIATN,
            DEVIATN::default().deviation_m(1).deviation_e(4).bits(),
        )
        .unwrap();

    cc1101
        .set_autocalibration(AutoCalibration::FromIdle)
        .unwrap();
}

/// Same as config_0, but:
/// - Base Frequency 902.5Mhz (0x216D0A)
/// - Filter Bandwidth 562.5kHz (m=2,e=0)
/// - Data Rate 250kBaud (m=48,e=13)
/// - IF 316.4kHz (12)
/// - Modulation GFSK (001)
/// - Deviation 132kHz (m=2,e=6)
/// - Channel spacing 421kHz (Max) (m=255,e=3)
pub fn config_1<T: SpiDevice>(cc1101: &mut Cc1101<T>) {
  // Set carrier base frequency 902.5 MHz
  cc1101.0.write_register(Config::FREQ2, 0x21).unwrap();
  cc1101.0.write_register(Config::FREQ1, 0x6D).unwrap();
  cc1101.0.write_register(Config::FREQ0, 0x0A).unwrap();

  // Set power level to max
  cc1101
      .0
      .write_register(Config::FREND0, FREND0::default().pa_power(7).bits())
      .unwrap();

  cc1101.0.write_register(Config::CHANNR, 31).unwrap();

  // Set IF 316.4kHz
  cc1101
      .0
      .write_register(Config::FSCTRL1, FSCTRL1::default().freq_if(12).bits())
      .unwrap();

  // Set filter bandwidth to 562.5kHz, Data rate 250kBaud
  cc1101
      .0
      .write_register(
          Config::MDMCFG4,
          MDMCFG4::default()
              .chanbw_m(2)
              .chanbw_e(0)
              .drate_e(13)
              .bits(),
      )
      .unwrap();
  cc1101
      .0
      .write_register(Config::MDMCFG3, MDMCFG3::default().drate_m(48).bits())
      .unwrap();

  // Set channel spacing to 421kHz (max)
  cc1101
      .0
      .write_register(
          Config::MDMCFG1,
          MDMCFG1::default()
              .chanspc_e(3)
              .fec_en(1)
              .num_preamble(7)
              .bits(),
      )
      .unwrap();
  cc1101
      .0
      .write_register(Config::MDMCFG0, MDMCFG0::default().chanspc_m(255).bits())
      .unwrap();
  // Set modulation to GFSK
  cc1101
      .0
      .write_register(Config::MDMCFG2, MDMCFG2::default().mod_format(1).bits())
      .unwrap();
  // Set deviation to 132kHz
  cc1101
      .0
      .write_register(
          Config::DEVIATN,
          DEVIATN::default().deviation_m(2).deviation_e(6).bits(),
      )
      .unwrap();

  cc1101
      .0
      .write_patable(&[0x03, 0x0E, 0x1E, 0x27, 0x8E, 0xCD, 0xC7, 0xC0])
      .unwrap();
  cc1101
      .0
      .write_register(
          Config::PKTCTRL0,
          PKTCTRL0::default().crc_en(1).white_data(1).bits(),
      )
      .unwrap();

  // Keep radio in rx mode even after packet received, make CCA always on
  cc1101
      .0
      .write_register(Config::MCSM1, MCSM1::default().rxoff_mode(3).cca_mode(0).bits())
      .unwrap();

  cc1101
      .0
      .write_register(
          Config::PKTCTRL1,
          PKTCTRL1::default()
              .pqt(4)
              .crc_autoflush(1)
              .append_status(0)
              .bits(),
      )
      .unwrap();

  cc1101
      .set_autocalibration(AutoCalibration::FromIdle)
      .unwrap();
  cc1101.set_packet_length(PacketLength::Fixed(32)).unwrap();

  cc1101.0.write_register(Config::IOCFG2, GdoCfg::CRC_OK.value()).unwrap();
}
