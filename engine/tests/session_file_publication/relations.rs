use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_rejects_original_source_registry_grant_upload_journal_and_target_pins(
) {
    for deferred in [false, true] {
        for mode in [
            "source",
            "registry",
            "grant",
            "upload",
            "journal",
            "publication",
        ] {
            let f = Fixture::ready(17).await;
            f.grants.issue(1, f.actor()).await;
            let mut a = prepare(&f, 1).await;
            let (schema,tables,pin_update)=match mode {
            "source"=>(&f.grants.auth.base.schema,vec!["users"],String::new()),
            "registry"=>(&f.grants.auth.base.schema,vec!["collaboration_identity_schema","collaboration_principals"],String::new()),
            "grant"=>(&f.grants.schema,vec![grant_fixture::MARKER,grant_fixture::CHANGES],format!("UPDATE {}.{} SET schema_oid='{}.{}'::regclass::oid::bigint,changes_oid='{}.{}'::regclass::oid::bigint;",f.grants.schema,grant_fixture::MARKER,f.grants.schema,grant_fixture::MARKER,f.grants.schema,grant_fixture::CHANGES)),
            "upload"=>(&f.upload_schema,vec!["collaboration_file_upload_schema","collaboration_file_uploads"],format!("UPDATE {}.collaboration_file_upload_schema SET schema_oid='{}.collaboration_file_upload_schema'::regclass::oid::bigint,uploads_oid='{}.collaboration_file_uploads'::regclass::oid::bigint;",f.upload_schema,f.upload_schema,f.upload_schema)),
            "journal"=>(&f.journal_schema,vec!["collaboration_file_upload_chunk_schema","collaboration_file_upload_chunks"],format!("UPDATE {}.collaboration_file_upload_chunk_schema SET schema_oid='{}.collaboration_file_upload_chunk_schema'::regclass::oid::bigint,chunks_oid='{}.collaboration_file_upload_chunks'::regclass::oid::bigint;",f.journal_schema,f.journal_schema,f.journal_schema)),
            _=>(&f.publication_schema,vec![PUBLICATION_MARKER],format!("UPDATE {}.{PUBLICATION_MARKER} SET schema_oid='{}.{PUBLICATION_MARKER}'::regclass::oid::bigint;",f.publication_schema,f.publication_schema)),
        };
            let mut body = String::new();
            for table in tables {
                body.push_str(&format!("ALTER TABLE {schema}.{table} RENAME TO original_{table}; CREATE TABLE {schema}.{table}(LIKE {schema}.original_{table} INCLUDING ALL); INSERT INTO {schema}.{table} SELECT * FROM {schema}.original_{table};"));
            }
            body.push_str(&pin_update);
            body.push_str("RETURN NEW;");
            fault(&f, &body, deferred).await;
            let before = f.snapshot().await;
            let result = publish(&f, &mut a).await;
            let state = a.state();
            let reached = hit(&f).await;
            let after = f.snapshot().await;
            remove_fault(&f).await;
            let healthy_result = healthy(&f).await;
            receipt(&healthy_result, 1, f.actor(), &f.manifest);
            f.cleanup().await;
            assert!(
                reached,
                "actual legal clone replacement {mode} deferred={deferred}"
            );
            assert!(
                result.is_err(),
                "new names/valid shape/new OID claims cannot replace captured relations"
            );
            assert_eq!(state, FileUploadPublicationState::Unknown);
            assert_eq!(
                after, before,
                "original tables/OIDs and tentative publication must survive exact rollback"
            );
        }
    }
}
