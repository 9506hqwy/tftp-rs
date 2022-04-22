use bytes::{BufMut, Bytes, BytesMut};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Options {
    blksize: Option<u16>,
    timeout: Option<u8>,
    tsize: Option<u64>,
    windowsize: Option<u16>,
}

impl Options {
    pub fn blksize(&self) -> usize {
        self.blksize.unwrap_or(512) as usize
    }

    pub fn timeout(&self) -> u64 {
        self.timeout.unwrap_or(10) as u64
    }

    pub fn tsize(&self) -> u64 {
        self.tsize.unwrap_or(0)
    }

    pub fn windowsize(&self) -> u16 {
        self.windowsize.unwrap_or(1)
    }

    pub fn as_bytes(&self) -> Bytes {
        let mut bytes = BytesMut::new();

        if let Some(blksize) = self.blksize {
            bytes.put("blksize".as_bytes());
            bytes.put_u8(0);

            bytes.put(blksize.to_string().as_bytes());
            bytes.put_u8(0);
        }

        if let Some(timeout) = self.timeout {
            bytes.put("timeout".as_bytes());
            bytes.put_u8(0);

            bytes.put(timeout.to_string().as_bytes());
            bytes.put_u8(0);
        }

        if let Some(tsize) = self.tsize {
            bytes.put("tsize".as_bytes());
            bytes.put_u8(0);

            bytes.put(tsize.to_string().as_bytes());
            bytes.put_u8(0);
        }

        if let Some(windowsize) = self.windowsize {
            bytes.put("windowsize".as_bytes());
            bytes.put_u8(0);

            bytes.put(windowsize.to_string().as_bytes());
            bytes.put_u8(0);
        }

        bytes.freeze()
    }

    pub fn cut_off(&mut self, limitations: &Options) {
        if let Some(blksize) = self.blksize {
            if limitations.blksize.map(|b| b < blksize).unwrap_or(false) {
                self.blksize = limitations.blksize;
            }
        }

        if limitations.timeout.is_none() {
            self.timeout = None;
        }

        if limitations.tsize.is_none() {
            self.tsize = None;
        }

        if let Some(windowsize) = self.windowsize {
            if limitations
                .windowsize
                .map(|w| w < windowsize)
                .unwrap_or(false)
            {
                self.windowsize = limitations.windowsize;
            }
        }
    }

    pub fn has_option(&self) -> bool {
        self.blksize.is_some()
            || self.timeout.is_some()
            || self.tsize.is_some()
            || self.windowsize.is_some()
    }

    pub fn set_tsize(&mut self, filepath: &Path) {
        if self.tsize.is_some() {
            self.tsize = Some(filepath.metadata().unwrap().len());
        }
    }
}

impl From<&mut Bytes> for Options {
    fn from(buf: &mut Bytes) -> Self {
        let mut options = Options::default();

        let mut parameters = buf.split(|&b| b == 0);

        loop {
            let key = parameters.next();
            if key.is_none() {
                break;
            }

            let value = parameters.next();
            if value.is_none() {
                break;
            }

            let k = String::from_utf8_lossy(key.unwrap());
            let v = String::from_utf8_lossy(value.unwrap());

            if k.to_lowercase() == "blksize" {
                if let Ok(blksize) = v.parse::<u16>() {
                    if (8..=65464).contains(&blksize) {
                        options.blksize = Some(blksize);
                    }
                }
            }

            if k.to_lowercase() == "timeout" {
                if let Ok(timeout) = v.parse::<u8>() {
                    if 1 <= timeout {
                        options.timeout = Some(timeout);
                    }
                }
            }

            if k.to_lowercase() == "tsize" {
                if let Ok(tsize) = v.parse::<u64>() {
                    options.tsize = Some(tsize);
                }
            }

            if k.to_lowercase() == "windowsize" {
                if let Ok(windowsize) = v.parse::<u16>() {
                    if 1 <= windowsize {
                        options.windowsize = Some(windowsize);
                    }
                }
            }
        }

        options
    }
}

#[derive(Default)]
pub struct OptionBuilder {
    options: Options,
}

impl OptionBuilder {
    pub fn blksize(self, blksize: u16) -> Self {
        OptionBuilder {
            options: Options {
                blksize: Some(blksize),
                ..self.options
            },
        }
    }

    pub fn timeout(self, timeout: u8) -> Self {
        OptionBuilder {
            options: Options {
                timeout: Some(timeout),
                ..self.options
            },
        }
    }

    pub fn tsize(self) -> Self {
        OptionBuilder {
            options: Options {
                tsize: Some(0),
                ..self.options
            },
        }
    }

    pub fn windowsize(self, windowsize: u16) -> Self {
        OptionBuilder {
            options: Options {
                windowsize: Some(windowsize),
                ..self.options
            },
        }
    }

    pub fn build(self) -> Options {
        self.options
    }
}
