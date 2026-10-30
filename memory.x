MEMORY
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* nRF52840 with the Adafruit nRF52 (UF2) bootloader, as on the eyelash
     halves and the XIAO BLE. RMK reclaims the SoftDevice region. */
  FLASH : ORIGIN = 0x00001000, LENGTH = 1020K
  RAM : ORIGIN = 0x20000008, LENGTH = 255K
}
