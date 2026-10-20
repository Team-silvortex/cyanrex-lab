use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*, platform_host::PlatformTaskReplaceConnector,
    services::task_store::TaskContentSnapshot,
};

pub(super) async fn run(
    p: &Prepared<'_>,
    connector: &PlatformTaskReplaceConnector,
    old: &TaskContentSnapshot,
) {
    let mut artifact = p.artifact(0, "第一版讲义😀\r\n".as_bytes()).await;
    let mut references = vec![artifact.reference.clone()];
    for index in 1..32 {
        artifact = p
            .revise(&artifact, format!("协作笔记第 {index} 版\t😀\n").as_bytes())
            .await;
        references.push(artifact.reference.clone());
    }
    references.reverse();
    let files = p.files();
    let immutable = p.immutable().await;
    let requested = TaskContentManifest::new(
        "32 个固定修订，不需要执行",
        references
            .iter()
            .map(|r| {
                TextPayloadBinding::new(r.clone(), "重复的讲义😀.txt", "future.language").unwrap()
            })
            .collect(),
    )
    .unwrap();
    let added = p
        .replace(connector, 0, old, requested.clone())
        .await
        .unwrap();
    assert_eq!(added.manifest, requested);
    assert_eq!(u64::from(added.task.revision), 3);
    p.reopened(&added).await;
    references.reverse();
    let requested = TaskContentManifest::new(
        "同一内容集合，原顺序和标签可以修改",
        references
            .iter()
            .map(|r| TextPayloadBinding::new(r.clone(), "新的显示名称.txt", "plaintext").unwrap())
            .collect(),
    )
    .unwrap();
    let reordered = p
        .replace(connector, 0, &added, requested.clone())
        .await
        .unwrap();
    assert_eq!(reordered.manifest, requested);
    assert_eq!(u64::from(reordered.task.revision), 4);
    p.reopened(&reordered).await;
    let empty = p
        .replace(
            connector,
            0,
            &reordered,
            manifest("保留协作任务，移除所有内容项", vec![]),
        )
        .await
        .unwrap();
    assert_eq!(u64::from(empty.task.revision), 5);
    p.unchanged(&empty, 5).await;
    p.reopened(&empty).await;
    assert_eq!(files, p.files());
    assert_eq!(immutable, p.immutable().await);
}
