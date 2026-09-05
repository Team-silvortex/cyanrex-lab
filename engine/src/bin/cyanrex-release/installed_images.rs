use crate::release_archive::lowercase_hex;
use serde_json::Value;
use std::io;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

const IMAGE_NAMES: [&str; 3] = ["engine", "frontend", "postgres"];
const MAX_DOCKER_OUTPUT_BYTES: u64 = 16 * 1024;

pub async fn verify(metadata: &Value) -> Result<(), String> {
    for name in IMAGE_NAMES {
        let reference = metadata["images"]["references"][name]
            .as_str()
            .ok_or_else(|| format!("release metadata {name} image reference is invalid"))?;
        let expected = metadata["images"]["contentIds"][name]
            .as_str()
            .ok_or_else(|| format!("release metadata {name} image content ID is invalid"))?;
        let actual = inspect_docker_image(reference).await?;
        if actual != expected {
            return Err(format!(
                "loaded {name} image ID {actual} does not match {expected}"
            ));
        }
    }
    Ok(())
}

async fn inspect_docker_image(reference: &str) -> Result<String, String> {
    let mut child = Command::new("docker")
        .args(["image", "inspect", "--format", "{{.Id}}", "--", reference])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("cannot inspect loaded image {reference:?}: {error}"))?;
    let stdout = child.stdout.take().expect("piped Docker stdout");
    let stderr = child.stderr.take().expect("piped Docker stderr");
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        tokio::try_join!(child.wait(), read_limited(stdout), read_limited(stderr))
    })
    .await;
    let (status, stdout, stderr) = match result {
        Ok(Ok(output)) => output,
        result => {
            let _ = child.kill().await;
            return Err(match result {
                Err(_) => format!("docker image inspection timed out for {reference:?}"),
                Ok(Err(error)) => format!("cannot inspect loaded image {reference:?}: {error}"),
                Ok(Ok(_)) => unreachable!(),
            });
        }
    };
    if !status.success() {
        return Err(format!(
            "cannot inspect loaded image {reference:?}: {}",
            output_excerpt(&stderr)
        ));
    }
    let value = std::str::from_utf8(&stdout)
        .map_err(|_| format!("docker returned a non-UTF-8 image ID for {reference:?}"))?
        .trim()
        .to_ascii_lowercase();
    if !value
        .strip_prefix("sha256:")
        .is_some_and(|hash| lowercase_hex(hash, &[64]))
    {
        return Err(format!(
            "docker returned an invalid image ID for {reference:?}"
        ));
    }
    Ok(value)
}

async fn read_limited(reader: impl AsyncRead + Unpin) -> io::Result<Vec<u8>> {
    let mut source = Vec::new();
    reader
        .take(MAX_DOCKER_OUTPUT_BYTES + 1)
        .read_to_end(&mut source)
        .await?;
    if source.len() as u64 > MAX_DOCKER_OUTPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "docker image inspection output is too large",
        ));
    }
    Ok(source)
}

fn output_excerpt(source: &[u8]) -> String {
    String::from_utf8_lossy(source)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\t' | '\n'))
        .take(512)
        .collect::<String>()
        .trim()
        .to_owned()
}
