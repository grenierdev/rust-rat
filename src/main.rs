use futures::FutureExt;
use std::env;
use std::error::Error;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process;

#[cfg(target_os = "windows")]
static SHELL_PROGRAM: &str = "powershell.exe";
#[cfg(not(target_os = "windows"))]
static SHELL_PROGRAM: &str = "bash";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8081".to_string());

	let listener = TcpListener::bind(listen_addr.clone()).await?;

	println!("Listening {}", listen_addr);

	while let Ok((inbound, client_addr)) = listener.accept().await {
		let transfer = transfer(inbound, client_addr.clone()).map(|r| {
            if let Err(e) = r {
                println!("Failed to transfer; error={}", e);
            }
        });

        tokio::spawn(transfer);
	}

	Ok(())
}

async fn transfer(mut inbound: TcpStream, client_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
	println!("Hello {}", client_addr);

	let (mut tcp_in, mut tcp_out) = inbound.split();

	let mut cmd = process::Command::new(SHELL_PROGRAM);
	cmd.kill_on_drop(true);
	cmd.stdin(std::process::Stdio::piped());
	cmd.stdout(std::process::Stdio::piped());
	cmd.stderr(std::process::Stdio::piped());

	if let Ok(mut child) = cmd.spawn() {
		if let (Some(mut child_in), Some(child_out), Some(child_err)) = (child.stdin.take(), child.stdout.take(), child.stderr.take()) {
			let mut child_out_err = child_out.chain(child_err);
			let tcp_to_cmd = async {
				tokio::io::copy(&mut tcp_in, &mut child_in).await?;
				child_in.shutdown().await
			};

			let child_to_tcp = async {
				tokio::io::copy(&mut child_out_err, &mut tcp_out).await?;
				tcp_out.shutdown().await
			};

			tokio::try_join!(tcp_to_cmd, child_to_tcp)?;
		}
	}

	println!("Bye {}", client_addr);
	Ok(())
}