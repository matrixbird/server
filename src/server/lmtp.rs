use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn start(bind: String) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(&bind).await?;
    println!("LMTP server listening on {}", bind);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("Connection from: {}", addr);

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut lines = BufReader::new(reader).lines();

            writer.write_all(b"220 localhost LMTP ready\r\n").await.ok()?;

            let mut mail_from = String::new();
            let mut rcpt_to = String::new();
            let mut data = String::new();
            let mut in_data = false;

            while let Some(line) = lines.next_line().await.ok().flatten() {
                let line = line.trim_end();
                if in_data {
                    if line == "." {
                        in_data = false;
                        println!("=== New Email ===");
                        println!("From: {}", mail_from);
                        println!("To: {}", rcpt_to);
                        println!("Data:\n{}", data);
                        println!("=================\n");

                        writer.write_all(b"250 2.1.5 OK\r\n").await.ok()?;
                        data.clear();
                    } else {
                        data.push_str(line);
                        data.push('\n');
                    }
                    continue;
                }

                if line.starts_with("LHLO") {
                    writer.write_all(b"250-localhost\r\n250-PIPELINING\r\n250 ENHANCEDSTATUSCODES\r\n").await.ok()?;
                } else if line.starts_with("MAIL FROM:") {
                    mail_from = line["MAIL FROM:".len()..].trim().to_string();
                    writer.write_all(b"250 2.1.0 OK\r\n").await.ok()?;
                } else if line.starts_with("RCPT TO:") {
                    rcpt_to = line["RCPT TO:".len()..].trim().to_string();
                    writer.write_all(b"250 2.1.5 OK\r\n").await.ok()?;
                } else if line == "DATA" {
                    writer.write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n").await.ok()?;
                    in_data = true;
                } else if line == "QUIT" {
                    writer.write_all(b"221 2.0.0 Bye\r\n").await.ok()?;
                    break;
                } else {
                    writer.write_all(b"502 5.5.2 Command not recognized\r\n").await.ok()?;
                }
            }

            Some(())
        });
    }
}


