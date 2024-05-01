//! Low level unrestricted access to the CC1101 radio chip.

use hal::spi::{Operation, SpiDevice};

#[macro_use]
mod macros;
mod access;
mod traits;

pub mod convert;
pub mod registers;
pub mod types;

use self::registers::*;

pub const FXOSC: u64 = 27_000_000;

pub struct Cc1101<SPI> {
    pub(crate) spi: SPI,
    //    gdo0: GDO0,
    //    gdo2: GDO2,
}

impl<SPI, SpiE> Cc1101<SPI>
where
    SPI: SpiDevice<u8, Error = SpiE>,
{
    pub fn new(spi: SPI) -> Result<Self, SpiE> {
        let cc1101 = Cc1101 {
            spi,
        };
        Ok(cc1101)
    }

    pub fn read_register<R>(&mut self, reg: R) -> Result<u8, SpiE>
    where
        R: Into<Register>,
    {
        let mut buffer = [reg.into().raddr(), 0u8];
        self.spi.transfer_in_place(&mut buffer)?;
        Ok(buffer[1])
    }

    fn read_burst(&mut self, addr: u8, buf: &mut [u8]) -> Result<(), SpiE> {
        let mut buffer = [addr | 0b1100_0000];
        self.spi
            .transaction(&mut [Operation::TransferInPlace(&mut buffer), Operation::Read(buf)])?;
        Ok(())
    }
    fn write_burst(&mut self, addr: u8, buf: &[u8]) -> Result<(), SpiE> {
        let mut buffer = [addr | 0b0100_0000];
        self.spi
            .transaction(&mut [Operation::TransferInPlace(&mut buffer), Operation::Write(buf)])?;
        Ok(())
    }

    /// The FIFO is 64 bytes long
    pub fn read_fifo(&mut self, buf: &mut [u8]) -> Result<(), SpiE> {
        self.read_burst(Command::FIFO.addr(), buf)
    }
    /// The FIFO is 64 bytes long
    pub fn write_fifo(&mut self, buf: &[u8]) -> Result<(), SpiE> {
        self.write_burst(Command::FIFO.addr(), buf)
    }
    /// The PATABLE is 8 bytes long
    pub fn read_patable(&mut self, buf: &mut [u8]) -> Result<(), SpiE> {
        self.read_burst(Command::PATABLE.addr(), buf)
    }
    /// The PATABLE is 8 bytes long
    pub fn write_patable(&mut self, buf: &[u8]) -> Result<(), SpiE> {
        self.write_burst(Command::PATABLE.addr(), buf)
    }

    pub fn write_strobe(&mut self, com: Command) -> Result<(), SpiE> {
        self.spi.write(&[com.addr()])?;
        Ok(())
    }
    /// Sends a NoOp to read status byte
    /// 
    /// Returns wether chip is ready to accept commands (when chip_rdyn (bit 7) is low (false))
    pub fn chip_rdyn(&mut self) -> Result<bool, SpiE> {
        let mut c = [Command::SNOP.addr()];
        self.spi.transfer_in_place(&mut c)?;
        Ok(c[0] & 0x80 == 0)
    }

    pub fn write_register<R>(&mut self, reg: R, byte: u8) -> Result<(), SpiE>
    where
        R: Into<Register>,
    {
        self.spi.write(&[reg.into().waddr(), byte])?;
        Ok(())
    }

    pub fn modify_register<R, F>(&mut self, reg: R, f: F) -> Result<(), SpiE>
    where
        R: Into<Register> + Copy,
        F: FnOnce(u8) -> u8,
    {
        let r = self.read_register(reg)?;
        self.write_register(reg, f(r))?;
        Ok(())
    }
}
