//! Outbound STARTTLS (RFC 3207): the client must upgrade to TLS when the peer
//! advertises STARTTLS, re-EHLO over the encrypted channel, and deliver — and
//! must still deliver in cleartext against a server that does not offer it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use alo_smtp_client::client::{OutboundSession, RcptOutcome};
use rustls::ServerConfig;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// A minimal SMTP server that offers STARTTLS, upgrades, and accepts one
/// message over TLS. Panics on any deviation so the test fails loudly.
async fn tls_mock() -> std::net::SocketAddr {
    let cert = rcgen::generate_simple_self_signed(vec!["mock.local".to_owned()]).unwrap();
    let cert_der = cert.cert.der().clone();
    let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server_config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], PrivateKeyDer::Pkcs8(key_der))
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (stream, _peer) = listener.accept().await.unwrap();
        serve(stream, acceptor).await;
    });
    addr
}

async fn read_line(reader: &mut (impl AsyncBufReadExt + Unpin)) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    line
}

async fn serve(stream: TcpStream, acceptor: TlsAcceptor) {
    let mut reader = BufReader::new(stream);
    reader.get_mut().write_all(b"220 mock.local ESMTP\r\n").await.unwrap();

    let ehlo = read_line(&mut reader).await;
    assert!(ehlo.to_uppercase().starts_with("EHLO"), "expected EHLO, got {ehlo:?}");
    // Advertise STARTTLS.
    reader
        .get_mut()
        .write_all(b"250-mock.local\r\n250 STARTTLS\r\n")
        .await
        .unwrap();

    let starttls = read_line(&mut reader).await;
    assert_eq!(starttls.trim_end(), "STARTTLS");
    reader.get_mut().write_all(b"220 go ahead\r\n").await.unwrap();
    // The client must not have pipelined anything past STARTTLS.
    assert!(reader.buffer().is_empty(), "client sent data before TLS");

    // Upgrade — from here everything is encrypted.
    let tcp = reader.into_inner();
    let tls = acceptor.accept(tcp).await.expect("TLS handshake");
    let mut tls = BufReader::new(tls);

    let ehlo2 = read_line(&mut tls).await;
    assert!(ehlo2.to_uppercase().starts_with("EHLO"), "expected EHLO over TLS, got {ehlo2:?}");
    tls.get_mut().write_all(b"250 mock.local\r\n").await.unwrap();

    let mail = read_line(&mut tls).await;
    assert!(mail.to_uppercase().starts_with("MAIL FROM"), "got {mail:?}");
    tls.get_mut().write_all(b"250 2.1.0 ok\r\n").await.unwrap();

    let rcpt = read_line(&mut tls).await;
    assert!(rcpt.to_uppercase().starts_with("RCPT TO"), "got {rcpt:?}");
    tls.get_mut().write_all(b"250 2.1.5 ok\r\n").await.unwrap();

    let data = read_line(&mut tls).await;
    assert_eq!(data.trim_end(), "DATA");
    tls.get_mut().write_all(b"354 go ahead\r\n").await.unwrap();

    // Drain the message body up to the CRLF.CRLF terminator.
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        tls.read_exact(&mut byte).await.unwrap();
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n.\r\n") {
            break;
        }
    }
    tls.get_mut().write_all(b"250 2.0.0 accepted\r\n").await.unwrap();
}

#[tokio::test]
async fn delivers_over_starttls() {
    let addr = tls_mock().await;
    let mut session = OutboundSession::connect_addr(addr, "client.local")
        .await
        .expect("connect + STARTTLS upgrade");
    assert!(session.is_tls(), "session must be encrypted after STARTTLS");

    let outcomes = session
        .deliver(
            Some("sender@client.local"),
            &["rcpt@mock.local".to_owned()],
            b"From: sender@client.local\r\nSubject: tls\r\n\r\nbody\r\n",
        )
        .await
        .expect("delivery over TLS");
    assert_eq!(outcomes, vec![RcptOutcome::Delivered]);
}
