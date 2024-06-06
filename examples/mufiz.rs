use std::path::PathBuf;
use quickfetch::Fetcher;
use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()>{
    let urls = vec!["https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.aarch64.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.i386.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.mips.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.mips64.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.mips64el.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.mipsel.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.ppc.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.ppc64.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.ppc64le.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.riscv64.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz-0.6.0-1.x86_64.rpm", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-linux-gnu.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-linux-musl.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-linux.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-macos.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-windows-gnu.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_aarch64-windows.zip", "https://github.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_amd64.deb", "https://github.com/Mustafif/MufiZ/releases/download
/v0.6.0/mufiz_0.6.0_arm-linux-gnueabihf.zip", "https
://github.com/Mustafif/MufiZ/releases/download/v0.6.
0/mufiz_0.6.0_arm-linux-musleabihf.zip", "https://gi
thub.com/Mustafif/MufiZ/releases/download/v0.6.0/muf
iz_0.6.0_arm64.deb", "https://github.com/Mustafif/Mu
fiZ/releases/download/v0.6.0/mufiz_0.6.0_i386.deb",
                    "https://github.com/Mustafif/MufiZ/releases/download
/v0.6.0/mufiz_0.6.0_mips-linux-musl.zip", "https://g
ithub.com/Mustafif/MufiZ/releases/download/v0.6.0/mu
fiz_0.6.0_mips.deb", "https://github.com/Mustafif/Mu
fiZ/releases/download/v0.6.0/mufiz_0.6.0_mips64-linu
x-musl.zip", "https://github.com/Mustafif/MufiZ/rele
ases/download/v0.6.0/mufiz_0.6.0_mips64.deb", "https
://github.com/Mustafif/MufiZ/releases/download/v0.6.
0/mufiz_0.6.0_mips64el-linux-musl.zip", "https://git
hub.com/Mustafif/MufiZ/releases/download/v0.6.0/mufi
z_0.6.0_mips64el.deb", "https://github.com/Mustafif/
MufiZ/releases/download/v0.6.0/mufiz_0.6.0_mipsel-li
nux-musl.zip", "https://github.com/Mustafif/MufiZ/re
leases/download/v0.6.0/mufiz_0.6.0_mipsel.deb", "htt
ps://github.com/Mustafif/MufiZ/releases/download/v0.
6.0/mufiz_0.6.0_powerpc-linux-musl.zip", "https://gi
thub.com/Mustafif/MufiZ/releases/download/v0.6.0/muf
iz_0.6.0_powerpc-linux.zip", "https://github.com/Mus
tafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_pow
erpc.deb", "https://github.com/Mustafif/MufiZ/releas
es/download/v0.6.0/mufiz_0.6.0_powerpc64-linux-gnu.z
ip", "https://github.com/Mustafif/MufiZ/releases/dow
nload/v0.6.0/mufiz_0.6.0_powerpc64-linux-musl.zip",
                    "https://github.com/Mustafif/MufiZ/releases/download
/v0.6.0/mufiz_0.6.0_powerpc64-linux.zip", "https://g
ithub.com/Mustafif/MufiZ/releases/download/v0.6.0/mu
fiz_0.6.0_powerpc64.deb", "https://github.com/Mustaf
if/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_powerp
c64le-linux-gnu.zip", "https://github.com/Mustafif/M
ufiZ/releases/download/v0.6.0/mufiz_0.6.0_powerpc64l
e-linux-musl.zip", "https://github.com/Mustafif/Mufi
Z/releases/download/v0.6.0/mufiz_0.6.0_powerpc64le-l
inux.zip", "https://github.com/Mustafif/MufiZ/releas
es/download/v0.6.0/mufiz_0.6.0_powerpc64le.deb", "ht
tps://github.com/Mustafif/MufiZ/releases/download/v0
.6.0/mufiz_0.6.0_riscv64-linux-musl.zip", "https://g
ithub.com/Mustafif/MufiZ/releases/download/v0.6.0/mu
fiz_0.6.0_riscv64-linux.zip", "https://github.com/Mu
stafif/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_ri
scv64.deb", "https://github.com/Mustafif/MufiZ/relea
ses/download/v0.6.0/mufiz_0.6.0_x86-linux-gnu.zip",
                    "https://github.com/Mustafif/MufiZ/releases/download
/v0.6.0/mufiz_0.6.0_x86-linux-musl.zip", "https://gi
thub.com/Mustafif/MufiZ/releases/download/v0.6.0/muf
iz_0.6.0_x86-linux.zip", "https://github.com/Mustafi
f/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_x86-win
dows-gnu.zip", "https://github.com/Mustafif/MufiZ/re
leases/download/v0.6.0/mufiz_0.6.0_x86_64-linux-gnu.
zip", "https://github.com/Mustafif/MufiZ/releases/do
wnload/v0.6.0/mufiz_0.6.0_x86_64-linux-musl.zip", "h
ttps://github.com/Mustafif/MufiZ/releases/download/v
0.6.0/mufiz_0.6.0_x86_64-linux.zip", "https://github
.com/Mustafif/MufiZ/releases/download/v0.6.0/mufiz_0
.6.0_x86_64-macos.zip", "https://github.com/Mustafif
/MufiZ/releases/download/v0.6.0/mufiz_0.6.0_x86_64-w
indows-gnu.zip", "https://github.com/Mustafif/MufiZ/
releases/download/v0.6.0/mufiz_0.6.0_x86_64-windows.zip"];

    let urls: Vec<String> = urls.iter().map(|x|x.to_string()).collect();
    let mut fetcher = Fetcher::new(&urls, "mufiz")?;
    fetcher.fetch().await?;
    fetcher.write_all(PathBuf::from("pkgs")).await?;
    Ok(())
}
