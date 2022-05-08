use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc;

pub(crate) fn rx() -> Receiver<String> {
    let (tx_stdin, rx_stdin) = mpsc::channel::<String>(10);
    // read from stdin
    let stdin_loop = async move {
        loop {
            let mut buf_stdin = tokio::io::BufReader::new(tokio::io::stdin());
            let mut line = String::new();
            buf_stdin.read_line(&mut line).await.unwrap();
            tx_stdin.send(line.trim().to_string()).await.unwrap();
            if line.trim() == "/exit" {
                break;
            }
        }
    };
    tokio::task::spawn(stdin_loop);
    rx_stdin
}