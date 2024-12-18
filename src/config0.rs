use crate::{configs::config_1, Cc1101, Error};
use hal::spi::SpiDevice;

use crate::lowlevel::types::*;

impl<SPI, SpiE> Cc1101<SPI>
where
	SPI: SpiDevice<u8, Error = SpiE>,
{
	/// If gdo2 pin is high, that means crc was successful
	/// and there's a valid packet we can read.
	/// Then just put that packet in the payload
	pub fn receive<P: hal::digital::InputPin>(
		&mut self,
		gdo2: &mut P,
	) -> nb::Result<[u8; 32], Error<SpiE>> {
		if gdo2.is_high().unwrap() {
			let mut payload = [0u8; 32];
			self.0
				.read_fifo(&mut payload)
				.map_err(|e| nb::Error::Other(e.into()))?;
			nb::Result::Ok(payload)
		} else {
			nb::Result::Err(nb::Error::WouldBlock)
		}
	}

	/// - write payload to FIFO
	/// - puts radio in transmit mode
	/// - waits for radio to go back to Idle
	/// - flushes the TX buffer
	pub fn transmit(&mut self, payload: &[u8; 32]) -> Result<(), Error<SpiE>> {
		// We go to iddle right before only if CCA isn't on mode 0
		// self.to_idle()?;
		self.0.write_fifo(payload)?;
		self.set_radio_mode(crate::RadioMode::Transmit)?;
		self.await_machine_state(MachineState::IDLE)?;
		self.flush_tx()?;
		Ok(())
	}
	/// We don't wait until radio is in TX.
	/// We just do the required steps for transmission to start.
	///
	/// - write payload to FIFO
	/// - sends command strobe for transmit mode
	pub fn transmit_start(&mut self, payload: &[u8; 32]) -> Result<(), Error<SpiE>> {
		self.0.write_fifo(payload)?;
		self.send_radio_mode_strobe(crate::RadioMode::Transmit)?;
		Ok(())
	}
	/// - waits for radio to go back to Iddle
	/// - flushes the TX buffer
	pub fn transmit_poll(&mut self) -> nb::Result<(), Error<SpiE>> {
		if self.is_state_machine(MachineState::IDLE)? {
			self.flush_tx()?;
			Ok(())
		} else {
			nb::Result::Err(nb::Error::WouldBlock)
		}
	}

	// Retransmissions and acks are a good idea but that would mean rethinking our packets
	// Because it would need a packet counter involved :( so that receiver doesn't get
	// same packet multiple times.

	// /// If packet received
	// /// After reception, keeps radio in rx mode
	// pub fn receive_with_acks<P: hal::digital::InputPin>(
	//     &mut self,
	//     gdo2: &mut P,
	// ) -> nb::Result<[u8; 32], Error<SpiE>> {
	// }

	// /// Transmits, then switches to rx and waits for ack payload
	// /// Ack payload is p[3] == 55, that doesn't mean
	// ///
	// /// - retries + 1 number of transmissions
	// pub fn transmit_with_retries<P: hal::digital::InputPin, D: hal::delay::DelayNs>(
	//     &mut self,
	//     payload: &[u8; 32],
	//     gdo2: &mut P,
	//     retries: u8,
	//     delay: &D
	// ) -> Result<bool, Error<SpiE>> {
	//     for i in 0..=retries {
	//         self.transmit(payload)?;
	//         self.to_rx()?;
	//         if let Ok(payload) = self.receive(gdo2) {
	//             if payload[3] == 55 {
	//                 self.to_idle()?;
	//                 return Ok(true)
	//             }
	//         }
	//     }
	//     Ok(false)
	// }

	pub fn configure(&mut self) -> Result<(), SpiE> {
		config_1(self);
		self.write_patable()?;
		Ok(())
	}
	pub fn write_patable(&mut self) -> Result<(), SpiE> {
		self.0
			.write_patable(&[0x03, 0x0E, 0x1E, 0x27, 0x8E, 0xCD, 0xC7, 0xC0])
	}
}
