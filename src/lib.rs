#![no_std]

extern crate embedded_hal as hal;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use core::fmt::{self, Display, Formatter};

use hal::spi::SpiDevice;

#[macro_use]
pub mod lowlevel;
pub mod config0;
mod configs;
pub mod rssi;

use lowlevel::convert::*;
pub use lowlevel::registers::*;
pub use lowlevel::types::*;
use rssi::rssi_to_dbm;

/// CC1101 errors.
#[derive(Debug)]
pub enum Error<SpiE> {
	/// Platform-dependent SPI-errors, such as IO errors.
	Spi(SpiE),
	/// The RX FIFO buffer overflowed, too small buffer for configured packet length.
	RxOverflow,
	/// Corrupt packet received with invalid CRC.
	CrcMismatch,
}

impl<SpiE> From<SpiE> for Error<SpiE> {
	fn from(e: SpiE) -> Self {
		Error::Spi(e)
	}
}

#[cfg(feature = "std")]
impl<SpiE: Display> Display for Error<SpiE> {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		match self {
			Self::RxOverflow => write!(f, "RX FIFO buffer overflowed"),
			Self::CrcMismatch => write!(f, "CRC mismatch"),
			Self::Spi(e) => write!(f, "SPI error: {}", e),
		}
	}
}

#[cfg(feature = "std")]
impl<SpiE: Display + core::fmt::Debug> std::error::Error for Error<SpiE> {}

/// High level API for interacting with the CC1101 radio chip.
pub struct Cc1101<SPI>(pub lowlevel::Cc1101<SPI>);

impl<SPI: SpiDevice<u8, Error = SpiE>, SpiE> Cc1101<SPI> {
	/// Make a new device, only returns an instance of Cc1101
	///
	/// You should:
	///  - `reset` the device right after
	///  - Wait some time (~1ms) for it to stabalize
	///  - Then `configure` it with the settings you'll be using
	pub fn new(spi: SPI) -> Result<Self, Error<SpiE>> {
		Ok(Cc1101(lowlevel::Cc1101::new(spi)?))
	}

	/// Sets the carrier frequency (in Hertz).
	pub fn set_frequency(&mut self, hz: u64) -> Result<(), Error<SpiE>> {
		let (freq0, freq1, freq2) = from_frequency(hz);
		self.0.write_register(Config::FREQ0, freq0)?;
		self.0.write_register(Config::FREQ1, freq1)?;
		self.0.write_register(Config::FREQ2, freq2)?;
		Ok(())
	}

	/// Sets the frequency synthesizer intermediate frequency (in Hertz).
	pub fn set_synthesizer_if(&mut self, hz: u64) -> Result<(), Error<SpiE>> {
		self.0.write_register(
			Config::FSCTRL1,
			FSCTRL1::default().freq_if(from_freq_if(hz)).bits(),
		)?;
		Ok(())
	}

	/// Sets the target value for the averaged amplitude from the digital channel filter.
	pub fn set_agc_target(&mut self, target: TargetAmplitude) -> Result<(), Error<SpiE>> {
		self.0.modify_register(Config::AGCCTRL2, |r| {
			AGCCTRL2(r).modify().magn_target(target.into()).bits()
		})?;
		Ok(())
	}

	/// Sets the filter length (in FSK/MSK mode) or decision boundary (in OOK/ASK mode) for the AGC.
	pub fn set_agc_filter_length(
		&mut self,
		filter_length: FilterLength,
	) -> Result<(), Error<SpiE>> {
		self.0.modify_register(Config::AGCCTRL0, |r| {
			AGCCTRL0(r)
				.modify()
				.filter_length(filter_length.into())
				.bits()
		})?;
		Ok(())
	}

	/// Configures when to run automatic calibration.
	pub fn set_autocalibration(&mut self, autocal: AutoCalibration) -> Result<(), Error<SpiE>> {
		self.0.modify_register(Config::MCSM0, |r| {
			MCSM0(r).modify().fs_autocal(autocal.into()).bits()
		})?;
		Ok(())
	}

	pub fn set_deviation(&mut self, deviation: u64) -> Result<(), Error<SpiE>> {
		let (mantissa, exponent) = from_deviation(deviation);
		self.0.write_register(
			Config::DEVIATN,
			DEVIATN::default()
				.deviation_m(mantissa)
				.deviation_e(exponent)
				.bits(),
		)?;
		Ok(())
	}

	/// Sets the data rate (in bits per second).
	pub fn set_data_rate(&mut self, baud: u64) -> Result<(), Error<SpiE>> {
		let (mantissa, exponent) = from_drate(baud);
		self.0.modify_register(Config::MDMCFG4, |r| {
			MDMCFG4(r).modify().drate_e(exponent).bits()
		})?;
		self.0
			.write_register(Config::MDMCFG3, MDMCFG3::default().drate_m(mantissa).bits())?;
		Ok(())
	}

	/// Sets the channel bandwidth (in Hertz).
	pub fn set_chanbw(&mut self, bandwidth: u64) -> Result<(), Error<SpiE>> {
		let (mantissa, exponent) = from_chanbw(bandwidth);
		self.0.modify_register(Config::MDMCFG4, |r| {
			MDMCFG4(r)
				.modify()
				.chanbw_m(mantissa)
				.chanbw_e(exponent)
				.bits()
		})?;
		Ok(())
	}

	pub fn get_hw_info(&mut self) -> Result<(u8, u8), Error<SpiE>> {
		let partnum = self.0.read_register(Status::PARTNUM)?;
		let version = self.0.read_register(Status::VERSION)?;
		Ok((partnum, version))
	}

	/// Received Signal Strength Indicator is an estimate of the signal power level in the chosen channel.
	pub fn get_rssi_dbm(&mut self) -> Result<i16, Error<SpiE>> {
		Ok(rssi_to_dbm(self.0.read_register(Status::RSSI)?))
	}

	/// The Link Quality Indicator metric of the current quality of the received signal.
	/// The CRC check for last packet.
	pub fn get_crc_lqi(&mut self) -> Result<(bool, u8), Error<SpiE>> {
		let lqi = LQI(self.0.read_register(Status::LQI)?);
		Ok((lqi.crc_ok() > 0, lqi.lqi()))
	}

	/// Configure the sync word to use, and at what level it should be verified.
	pub fn set_sync_mode(&mut self, sync_mode: SyncMode) -> Result<(), Error<SpiE>> {
		let reset: u16 = (SYNC1::default().bits() as u16) << 8 | (SYNC0::default().bits() as u16);

		let (mode, word) = match sync_mode {
			SyncMode::Disabled => (SyncCheck::DISABLED, reset),
			SyncMode::MatchPartial(word) => (SyncCheck::CHECK_15_16, word),
			SyncMode::MatchPartialRepeated(word) => (SyncCheck::CHECK_30_32, word),
			SyncMode::MatchFull(word) => (SyncCheck::CHECK_16_16, word),
		};
		self.0.modify_register(Config::MDMCFG2, |r| {
			MDMCFG2(r).modify().sync_mode(mode.value()).bits()
		})?;
		self.0
			.write_register(Config::SYNC1, ((word >> 8) & 0xff) as u8)?;
		self.0.write_register(Config::SYNC0, (word & 0xff) as u8)?;
		Ok(())
	}

	/// Configure signal modulation.
	pub fn set_modulation(&mut self, format: Modulation) -> Result<(), Error<SpiE>> {
		use lowlevel::types::ModFormat as MF;

		let value = match format {
			Modulation::BinaryFrequencyShiftKeying => MF::MOD_2FSK,
			Modulation::GaussianFrequencyShiftKeying => MF::MOD_GFSK,
			Modulation::OnOffKeying => MF::MOD_ASK_OOK,
			Modulation::FourFrequencyShiftKeying => MF::MOD_4FSK,
			Modulation::MinimumShiftKeying => MF::MOD_MSK,
		};
		self.0.modify_register(Config::MDMCFG2, |r| {
			MDMCFG2(r).modify().mod_format(value.value()).bits()
		})?;
		Ok(())
	}

	/// Configure device address, and address filtering.
	pub fn set_address_filter(&mut self, filter: AddressFilter) -> Result<(), Error<SpiE>> {
		use lowlevel::types::AddressCheck as AC;

		let (mode, addr) = match filter {
			AddressFilter::Disabled => (AC::DISABLED, ADDR::default().bits()),
			AddressFilter::Device(addr) => (AC::SELF, addr),
			AddressFilter::DeviceLowBroadcast(addr) => (AC::SELF_LOW_BROADCAST, addr),
			AddressFilter::DeviceHighLowBroadcast(addr) => (AC::SELF_HIGH_LOW_BROADCAST, addr),
		};
		self.0.modify_register(Config::PKTCTRL1, |r| {
			PKTCTRL1(r).modify().adr_chk(mode.value()).bits()
		})?;
		self.0.write_register(Config::ADDR, addr)?;
		Ok(())
	}

	/// Configure packet mode, and length.
	pub fn set_packet_length(&mut self, length: PacketLength) -> Result<(), Error<SpiE>> {
		use lowlevel::types::LengthConfig as LC;

		let (format, pktlen) = match length {
			PacketLength::Fixed(limit) => (LC::FIXED, limit),
			PacketLength::Variable(max_limit) => (LC::VARIABLE, max_limit),
			PacketLength::Infinite => (LC::INFINITE, PKTLEN::default().bits()),
		};
		self.0.modify_register(Config::PKTCTRL0, |r| {
			PKTCTRL0(r).modify().length_config(format.value()).bits()
		})?;
		self.0.write_register(Config::PKTLEN, pktlen)?;
		Ok(())
	}

	/// Set radio in Receive/Transmit/Idle/Calibrate mode.
	///
	/// Blocks until radio is in that mode.
	pub fn set_radio_mode(&mut self, radio_mode: RadioMode) -> Result<(), Error<SpiE>> {
		let target = self.send_radio_mode_strobe(radio_mode)?;
		self.await_machine_state(target)
	}
	#[cfg(feature = "tokio")]
	pub  async fn set_radio_mode_async(&mut self, radio_mode: RadioMode) -> Result<(), Error<SpiE>> {
		let target = self.send_radio_mode_strobe(radio_mode)?;
		self.await_machine_state(target)
	}
	/// Set radio mode but in 
	/// Send command strobe for Receive/Transmit/Idle/Calibrate mode.
	///
	/// Returns machine state for that RadioMode.
	pub fn send_radio_mode_strobe(
		&mut self,
		radio_mode: RadioMode,
	) -> Result<MachineState, Error<SpiE>> {
		Ok(match radio_mode {
			RadioMode::Receive => {
				// self.set_radio_mode(RadioMode::Idle)?;
				self.0.write_strobe(Command::SRX)?;
				MachineState::RX
			}
			RadioMode::Transmit => {
				// self.set_radio_mode(RadioMode::Idle)?;
				self.0.write_strobe(Command::STX)?;
				MachineState::TX
			}
			RadioMode::Idle => {
				self.0.write_strobe(Command::SIDLE)?;
				MachineState::IDLE
			}
			RadioMode::Calibrate => {
				self.set_radio_mode(RadioMode::Idle)?;
				self.0.write_strobe(Command::SCAL)?;
				MachineState::IDLE
			}
		})
	}

	/// Resets the chip.
	pub fn reset(&mut self) -> Result<(), Error<SpiE>> {
		Ok(self.0.write_strobe(Command::SRES)?)
	}
	pub fn flush_rx(&mut self) -> Result<(), Error<SpiE>> {
		Ok(self.0.write_strobe(Command::SFRX)?)
	}
	pub fn flush_tx(&mut self) -> Result<(), Error<SpiE>> {
		Ok(self.0.write_strobe(Command::SFTX)?)
	}
	/// Sends a no-op continuously
	///
	/// Blocks until chip is ready.
	pub fn wake_up_wait(&mut self) -> Result<(), Error<SpiE>> {
		while !(self.0.chip_rdyn()?) {}
		Ok(())
	}
	/// Enter pwr down mode when CSn goes high
	/// Remember that patable is lost afterwards
	/// so it has to be set again with `write_patable`
	pub fn power_down(&mut self) -> Result<(), Error<SpiE>> {
		Ok(self.0.write_strobe(Command::SPWD)?)
	}
	pub fn to_idle(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode(RadioMode::Idle)
	}
	pub fn to_tx(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode(RadioMode::Transmit)
	}
	pub fn to_rx(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode(RadioMode::Receive)
	}
	#[cfg(feature = "tokio")]
	pub async fn to_idle_async(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode_async(RadioMode::Idle).await
	}
	#[cfg(feature = "tokio")]
	pub async fn to_tx_async(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode_async(RadioMode::Transmit).await
	}
	#[cfg(feature = "tokio")]
	pub async fn to_rx_async(&mut self) -> Result<(), Error<SpiE>> {
		self.set_radio_mode_async(RadioMode::Receive).await
	}


	pub fn await_machine_state(&mut self, target: MachineState) -> Result<(), Error<SpiE>> {
		loop {
			if self.is_state_machine(target)? {
				break;
			}
		}
		Ok(())
	}
	#[cfg(feature = "tokio")]
	pub async fn await_machine_state_async(&mut self, target: MachineState) -> Result<(), Error<SpiE>> {
		let mut interval = tokio::time::interval(std::time::Duration::from_micros(100));
		interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
		interval.tick().await; // Let first instant tick happen
		loop {
			interval.tick().await;
			if self.is_state_machine(target)? {
				break;
			}
		}
		Ok(())
	}
	pub fn is_state_machine(&mut self, target: MachineState) -> Result<bool, Error<SpiE>> {
		Ok(target.value() == self.get_marc_state()?)
	}
	pub fn get_marc_state(&mut self) -> Result<u8, Error<SpiE>> {
		Ok(MARCSTATE(self.0.read_register(Status::MARCSTATE)?).marc_state())
	}
}

/// Modulation format configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Modulation {
	/// 2-FSK.
	BinaryFrequencyShiftKeying,
	/// GFSK.
	GaussianFrequencyShiftKeying,
	/// ASK / OOK.
	OnOffKeying,
	/// 4-FSK.
	FourFrequencyShiftKeying,
	/// MSK.
	MinimumShiftKeying,
}

/// Packet length configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PacketLength {
	/// Set packet length to a fixed value.
	Fixed(u8),
	/// Set upper bound of variable packet length.
	Variable(u8),
	/// Infinite packet length, streaming mode.
	Infinite,
}

/// Address check configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AddressFilter {
	/// No address check.
	Disabled,
	/// Address check, no broadcast.
	Device(u8),
	/// Address check and 0 (0x00) broadcast.
	DeviceLowBroadcast(u8),
	/// Address check and 0 (0x00) and 255 (0xFF) broadcast.
	DeviceHighLowBroadcast(u8),
}

/// Radio operational mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RadioMode {
	Receive,
	Transmit,
	Idle,
	Calibrate,
}

/// Sync word configuration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SyncMode {
	/// No sync word.
	Disabled,
	/// Match 15 of 16 bits of given sync word.
	MatchPartial(u16),
	/// Match 30 of 32 bits of a repetition of given sync word.
	MatchPartialRepeated(u16),
	/// Match 16 of 16 bits of given sync word.
	MatchFull(u16),
}

/// Target amplitude for AGC.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TargetAmplitude {
	/// 24 dB
	Db24 = 0,
	/// 27 dB
	Db27 = 1,
	/// 30 dB
	Db30 = 2,
	/// 33 dB
	Db33 = 3,
	/// 36 dB
	Db36 = 4,
	/// 38 dB
	Db38 = 5,
	/// 40 dB
	Db40 = 6,
	/// 42 dB
	Db42 = 7,
}

impl From<TargetAmplitude> for u8 {
	fn from(value: TargetAmplitude) -> Self {
		value as Self
	}
}

/// Channel filter samples or OOK/ASK decision boundary for AGC.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FilterLength {
	/// 8 filter samples for FSK/MSK, or 4 dB for OOK/ASK.
	Samples8 = 0,
	/// 16 filter samples for FSK/MSK, or 8 dB for OOK/ASK.
	Samples16 = 1,
	/// 32 filter samples for FSK/MSK, or 12 dB for OOK/ASK.
	Samples32 = 2,
	/// 64 filter samples for FSK/MSK, or 16 dB for OOK/ASK.
	Samples64 = 3,
}

impl From<FilterLength> for u8 {
	fn from(value: FilterLength) -> Self {
		value as Self
	}
}
