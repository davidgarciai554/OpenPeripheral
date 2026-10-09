//! Mouse photos from img.openmouse.app (plan `fotos-de-ratones`): the only
//! network access of the app, off with «Descargar fotos de los ratones».
//!
//! Only files from the fixed table below are ever requested, saved or
//! deleted: the name a device reports picks a row, but never becomes part
//! of a URL or a path. A photo is used only if its SHA-256 is the one in
//! the table (the server is a third party's and once served a wrong image),
//! so a photo replaced on the server falls back to the drawing until the
//! table is updated.
//!
//! Cached in `%APPDATA%\OpenPeripheral\images\`. The file's modification
//! time is the last time the app saw a mouse of that model; at start, a
//! photo not seen for 14 days is deleted. Limit: the time is per photo (per
//! model), not per device, so two mice of one model keep it while either is
//! seen; a per-device date in the config would be needed to tell them apart.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::Security::Cryptography::{BCRYPT_SHA256_ALG_HANDLE, BCryptHash};
use windows::core::{PCWSTR, w};

pub struct Photo {
    /// Whole words of the reported name, upper case (G304 and G305 share a
    /// shell).
    pub patterns: &'static [&'static str],
    pub file: &'static str,
    pub sha256: [u8; 32],
}

/// Only models whose buttons were placed on their photo
/// (`mouse_art::photo_art`); the rest keep the drawing.
pub static PHOTOS: &[Photo] = &[
    Photo {
        patterns: &["G304", "G305"],
        file: "logitech-g305.png",
        sha256: hex("a7c419758db244429292792e3d725b506c7901b923f1a0198322de861b400a07"),
    },
    Photo {
        patterns: &["G203"],
        file: "logitech-g203.png",
        sha256: hex("a07041557158c10ef8ce3207ebb973d11ee7869738685d76258e803023b831a2"),
    },
];

/// Larger downloads are cut; the G305 photo is 447 KB.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
/// Width and height checked in the PNG header before decoding.
pub const MAX_SIDE: u32 = 2048;
/// Days without seeing the model before its photo is deleted.
pub const MAX_AGE: Duration = Duration::from_secs(14 * 24 * 3600);

const fn hex(s: &str) -> [u8; 32] {
    const fn nibble(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("hex"),
        }
    }
    let b = s.as_bytes();
    assert!(b.len() == 64);
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = (nibble(b[2 * i]) << 4) | nibble(b[2 * i + 1]);
        i += 1;
    }
    out
}

/// The photo of a mouse by the name it reports.
pub fn photo_for(name: &str) -> Option<&'static Photo> {
    let name = name.to_uppercase();
    PHOTOS
        .iter()
        .find(|p| p.patterns.iter().any(|pat| has_word(&name, pat)))
}

/// `pat` in `text` with no letter or digit right before or after it.
fn has_word(text: &str, pat: &str) -> bool {
    let alnum = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    text.match_indices(pat).any(|(i, _)| {
        !alnum(text[..i].chars().next_back()) && !alnum(text[i + pat.len()..].chars().next())
    })
}

/// Width and height of a PNG within the limits, from its signature and
/// IHDR, without decoding it.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != SIGNATURE || bytes[8..16] != *b"\0\0\0\x0dIHDR" {
        return None;
    }
    let be = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let (w, h) = (be(16), be(20));
    ((1..=MAX_SIDE).contains(&w) && (1..=MAX_SIDE).contains(&h)).then_some((w, h))
}

/// SHA-256 with Windows' own implementation (BCrypt).
pub fn sha256(bytes: &[u8]) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, bytes, &mut out) }
        .is_ok()
        .then_some(out)
}

/// The bytes are this photo: size, PNG header and SHA-256.
pub fn valid(photo: &Photo, bytes: &[u8]) -> bool {
    bytes.len() <= MAX_BYTES && png_size(bytes).is_some() && sha256(bytes) == Some(photo.sha256)
}

/// `%APPDATA%\OpenPeripheral\images`.
pub fn dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("OpenPeripheral")
            .join("images"),
    )
}

fn tmp_name(photo: &Photo) -> String {
    format!("{}.tmp", photo.file)
}

/// The cached photo if it is valid; an invalid file is deleted.
pub fn load(dir: &Path, photo: &Photo) -> Option<Vec<u8>> {
    let path = dir.join(photo.file);
    let len = std::fs::metadata(&path).ok()?.len();
    let bytes = (len <= MAX_BYTES as u64)
        .then(|| std::fs::read(&path).ok())
        .flatten();
    match bytes {
        Some(b) if valid(photo, &b) => Some(b),
        _ => {
            let _ = std::fs::remove_file(&path);
            None
        }
    }
}

/// Marks the model as seen now (the age the sweep measures).
pub fn touch(dir: &Path, photo: &Photo) {
    if let Ok(f) = std::fs::File::options()
        .write(true)
        .open(dir.join(photo.file))
    {
        let _ = f.set_modified(SystemTime::now());
    }
}

/// At start: deletes leftovers of interrupted downloads and the photos not
/// seen for more than `max_age`. A time in the future counts as recent.
pub fn sweep(dir: &Path, now: SystemTime, max_age: Duration) {
    for p in PHOTOS {
        let _ = std::fs::remove_file(dir.join(tmp_name(p)));
        let path = dir.join(p.file);
        let old = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).is_ok_and(|age| age > max_age));
        if old {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// When the option is turned off: no photo is kept.
pub fn delete_all(dir: &Path) {
    for p in PHOTOS {
        let _ = std::fs::remove_file(dir.join(p.file));
        let _ = std::fs::remove_file(dir.join(tmp_name(p)));
    }
}

/// Where photos are downloaded from. Tests use a local server.
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub secure: bool,
    /// The system proxy settings (not for the local test server).
    pub proxy: bool,
    /// Each step (resolve, connect, send, each read).
    pub step: Duration,
    pub total: Duration,
}

impl Endpoint {
    pub fn openmouse() -> Self {
        Self {
            host: "img.openmouse.app".into(),
            port: 443,
            secure: true,
            proxy: true,
            step: Duration::from_secs(10),
            total: Duration::from_secs(30),
        }
    }
}

/// Downloads, checks and saves a photo. `keep` is asked right before the
/// file is put in place: false (the option was turned off) discards it.
pub fn download(
    endpoint: &Endpoint,
    photo: &Photo,
    dir: &Path,
    keep: &dyn Fn() -> bool,
) -> Result<(), &'static str> {
    let bytes = fetch(endpoint, &format!("/{}", photo.file))?;
    if !valid(photo, &bytes) {
        return Err("no es la foto esperada");
    }
    std::fs::create_dir_all(dir).map_err(|_| "no se pudo crear la carpeta")?;
    let tmp = dir.join(tmp_name(photo));
    let placed = std::fs::write(&tmp, &bytes).is_ok()
        && keep()
        && std::fs::rename(&tmp, dir.join(photo.file)).is_ok();
    if !placed {
        let _ = std::fs::remove_file(&tmp);
        return Err("no se guardó");
    }
    Ok(())
}

/// A WinHTTP handle, closed on drop.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn open(h: *mut c_void) -> Result<Handle, &'static str> {
    if h.is_null() {
        Err("WinHTTP")
    } else {
        Ok(Handle(h))
    }
}

/// GET with the certificate checked by Windows, no redirects, no cookies,
/// only 200 and at most `MAX_BYTES`, whatever Content-Length says.
fn fetch(endpoint: &Endpoint, path: &str) -> Result<Vec<u8>, &'static str> {
    let start = Instant::now();
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (host, path) = (wide(&endpoint.host), wide(path));
    let step = endpoint.step.as_millis().min(i32::MAX as u128) as i32;
    let access = if endpoint.proxy {
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
    } else {
        WINHTTP_ACCESS_TYPE_NO_PROXY
    };
    let err = |_| "red";
    unsafe {
        let session = open(WinHttpOpen(
            w!("OpenPeripheral"),
            access,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, step, step, step, step).map_err(err)?;
        let never = WINHTTP_OPTION_REDIRECT_POLICY_NEVER.to_ne_bytes();
        WinHttpSetOption(
            Some(session.0),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&never),
        )
        .map_err(err)?;
        let connection = open(WinHttpConnect(
            session.0,
            PCWSTR(host.as_ptr()),
            endpoint.port,
            0,
        ))?;
        let flags = if endpoint.secure {
            WINHTTP_FLAG_SECURE
        } else {
            WINHTTP_OPEN_REQUEST_FLAGS(0)
        };
        let request = open(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        ))?;
        let off = (WINHTTP_DISABLE_COOKIES | WINHTTP_DISABLE_REDIRECTS).to_ne_bytes();
        WinHttpSetOption(Some(request.0), WINHTTP_OPTION_DISABLE_FEATURE, Some(&off))
            .map_err(err)?;
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(err)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(err)?;
        let mut status = 0u32;
        let mut len = size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(err)?;
        if status != 200 {
            return Err("respuesta distinta de 200");
        }
        let mut body = Vec::new();
        let mut buf = [0u8; 16 * 1024];
        loop {
            if start.elapsed() > endpoint.total {
                return Err("tiempo agotado");
            }
            let mut n = 0u32;
            WinHttpReadData(request.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n)
                .map_err(err)?;
            if n == 0 {
                return Ok(body);
            }
            body.extend_from_slice(&buf[..n as usize]);
            if body.len() > MAX_BYTES {
                return Err("demasiado grande");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A fresh folder in the temp directory, removed on drop.
    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "openperipheral-photos-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A PNG header (8×4) followed by `extra` bytes: enough for the checks
    /// made before decoding.
    fn png(extra: usize) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(b"\0\0\0\x0dIHDR");
        b.extend_from_slice(&8u32.to_be_bytes());
        b.extend_from_slice(&4u32.to_be_bytes());
        b.extend(std::iter::repeat_n(7u8, extra));
        b
    }

    fn photo_of(bytes: &[u8]) -> Photo {
        Photo {
            patterns: &[],
            file: "test.png",
            sha256: sha256(bytes).unwrap(),
        }
    }

    /// Answers one request with `response` (written in pieces, then the
    /// connection stays open for `hold`).
    fn server(response: Vec<u8>, hold: Duration) -> Endpoint {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = s.write_all(&response);
                std::thread::sleep(hold);
            }
        });
        Endpoint {
            host: "127.0.0.1".into(),
            port,
            secure: false,
            proxy: false,
            step: Duration::from_millis(700),
            total: Duration::from_secs(3),
        }
    }

    fn http(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
        let mut r = format!("HTTP/1.1 {status}\r\n{headers}Connection: close\r\n\r\n").into_bytes();
        r.extend_from_slice(body);
        r
    }

    fn ok(body: &[u8]) -> Vec<u8> {
        http(
            "200 OK",
            &format!("Content-Length: {}\r\n", body.len()),
            body,
        )
    }

    #[test]
    fn the_name_picks_a_photo_of_the_table_and_nothing_else() {
        let g305 = Some("logitech-g305.png");
        let file = |n: &str| photo_for(n).map(|p| p.file);
        assert_eq!(file("G305 Lightspeed Wireless Gaming Mouse"), g305);
        assert_eq!(file("g304"), g305);
        assert_eq!(file("Logitech G305"), g305);
        assert_eq!(
            file("G203 LIGHTSYNC Gaming Mouse"),
            Some("logitech-g203.png")
        );
        for other in [
            "G2030",
            "G502 X",
            "G3050",
            "XG305",
            "",
            "../G30",
            "..\\..\\x",
            "/etc/passwd",
            "%2e%2e/",
            "?a=G30",
            "Ｇ305",
            "G30５",
        ] {
            assert_eq!(file(other), None, "{other}");
        }
    }

    #[test]
    fn table_hashes_and_bcrypt_sha256() {
        // FIPS 180-2 example.
        assert_eq!(
            sha256(b"abc").unwrap(),
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert!(PHOTOS.iter().all(|p| p.sha256 != [0; 32]));
    }

    #[test]
    fn the_png_header_is_checked_before_decoding() {
        assert_eq!(png_size(&png(0)), Some((8, 4)));
        let mut bad = png(0);
        bad[1] = b'X';
        assert_eq!(png_size(&bad), None);
        assert_eq!(png_size(&png(0)[..20]), None);
        let sized = |w: u32, h: u32| {
            let mut b = png(0);
            b[16..20].copy_from_slice(&w.to_be_bytes());
            b[20..24].copy_from_slice(&h.to_be_bytes());
            png_size(&b)
        };
        assert_eq!(sized(2048, 2048), Some((2048, 2048)));
        assert_eq!(sized(2049, 1), None);
        assert_eq!(sized(1, 2049), None);
        assert_eq!(sized(0, 5), None);
    }

    #[test]
    fn the_sweep_deletes_only_old_photos_of_the_table() {
        let d = Dir::new("sweep");
        let now = SystemTime::now();
        let path = d.0.join(PHOTOS[0].file);
        let tmp = d.0.join(tmp_name(&PHOTOS[0]));
        let other = d.0.join("mine.png");
        std::fs::write(&other, b"x").unwrap();
        let aged = |age: Duration, future: bool| {
            std::fs::write(&path, b"x").unwrap();
            let t = if future { now + age } else { now - age };
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(t)
                .unwrap();
            sweep(&d.0, now, MAX_AGE);
            path.exists()
        };
        let h = Duration::from_secs(3600);
        assert!(aged(MAX_AGE - h, false));
        assert!(aged(MAX_AGE, false));
        assert!(!aged(MAX_AGE + Duration::from_secs(1), false));
        assert!(aged(MAX_AGE * 2, true));
        std::fs::write(&tmp, b"x").unwrap();
        sweep(&d.0, now, MAX_AGE);
        assert!(!tmp.exists());
        assert!(other.exists());
        // Nothing to sweep is not an error.
        let _ = std::fs::remove_file(&path);
        sweep(&d.0, now, MAX_AGE);
        delete_all(&d.0);
        assert!(other.exists());
    }

    #[test]
    fn a_cached_photo_is_used_only_with_its_hash() {
        let d = Dir::new("load");
        let body = png(10);
        let photo = photo_of(&body);
        std::fs::write(d.0.join("test.png"), &body).unwrap();
        assert_eq!(load(&d.0, &photo), Some(body.clone()));
        std::fs::write(d.0.join("test.png"), png(11)).unwrap();
        assert_eq!(load(&d.0, &photo), None);
        assert!(!d.0.join("test.png").exists());
    }

    #[test]
    fn a_good_download_is_saved() {
        let d = Dir::new("ok");
        let body = png(100);
        let ep = server(ok(&body), Duration::ZERO);
        assert_eq!(download(&ep, &photo_of(&body), &d.0, &|| true), Ok(()));
        assert_eq!(std::fs::read(d.0.join("test.png")).unwrap(), body);
        assert!(!d.0.join("test.png.tmp").exists());
    }

    #[test]
    fn failed_downloads_leave_nothing() {
        let body = png(100);
        let photo = photo_of(&body);
        let mut other = png(100);
        other[30] = 0;
        let mut corrupt = body.clone();
        corrupt[0] = 0;
        let big = png(MAX_BYTES);
        let cases: Vec<(&str, Vec<u8>, Duration)> = vec![
            (
                "404",
                http("404 Not Found", "Content-Length: 0\r\n", b""),
                Duration::ZERO,
            ),
            ("otro hash", ok(&other), Duration::ZERO),
            ("no es PNG", ok(&corrupt), Duration::ZERO),
            (
                "redirección",
                http(
                    "302 Found",
                    "Location: http://127.0.0.1:1/test.png\r\nContent-Length: 0\r\n",
                    b"",
                ),
                Duration::ZERO,
            ),
            // No length (read until the server closes): cut at the limit.
            ("tope", http("200 OK", "", &big), Duration::ZERO),
            (
                "sin terminar",
                http(
                    "200 OK",
                    &format!("Content-Length: {}\r\n", body.len()),
                    &body[..50],
                ),
                Duration::from_secs(5),
            ),
        ];
        for (name, response, hold) in cases {
            let d = Dir::new(name.split(' ').next().unwrap());
            let ep = server(response, hold);
            let t = Instant::now();
            assert!(download(&ep, &photo, &d.0, &|| true).is_err(), "{name}");
            assert!(t.elapsed() < Duration::from_secs(4), "{name}");
            assert_eq!(std::fs::read_dir(&d.0).unwrap().count(), 0, "{name}");
        }
        // Turned off while downloading: nothing is kept.
        let d = Dir::new("off");
        let ep = server(ok(&body), Duration::ZERO);
        assert!(download(&ep, &photo, &d.0, &|| false).is_err());
        assert_eq!(std::fs::read_dir(&d.0).unwrap().count(), 0);
    }
}
