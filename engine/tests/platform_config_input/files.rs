use super::{
    fixture::{fifo, rejected, Fixture},
    json::document,
};
use std::{
    fs::{self, Permissions},
    os::unix::fs::{symlink, DirBuilderExt, PermissionsExt},
};

#[test]
fn platform_config_cli_rejects_unsafe_input_files_without_changing_parent_or_input() {
    let (outputs, unchanged) = Fixture::checked(|fixture| {
        let bytes = serde_json::to_vec(&document(&fixture.path("unused-artifacts"), 5432)).unwrap();
        let mut paths = Vec::new();
        let mut regular = Vec::new();
        for (index, mode) in [0o000, 0o644, 0o640, 0o620, 0o601, 0o700, 0o4600]
            .into_iter()
            .enumerate()
        {
            let path = fixture.write(&format!("bad-mode-{index}.json"), &bytes, mode);
            regular.push(path.clone());
            paths.push(path);
        }
        let hard = fixture.write("hard-target.json", &bytes, 0o600);
        let hard_alias = fixture.path("hard-alias.json");
        fs::hard_link(&hard, &hard_alias).unwrap();
        regular.push(hard.clone());
        paths.extend([hard, hard_alias]);
        let target = fixture.write("symlink-target.json", &bytes, 0o600);
        let link = fixture.path("symlink.json");
        symlink(&target, &link).unwrap();
        regular.push(target);
        paths.push(link);
        let pipe = fixture.path("pipe.json");
        fifo(&pipe);
        paths.push(pipe);
        let directory = fixture.path("directory.json");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        paths.push(directory);
        let long = fixture.write("too-long.json", &vec![b' '; 1024 * 1024], 0o600);
        paths.push(long.clone());
        paths.push(fixture.path("missing.json"));
        let mut outputs = paths
            .iter()
            .map(|path| fixture.run(&fixture.arguments(path), false))
            .collect::<Vec<_>>();
        let private_child = fixture.path("nonprivate-parent");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&private_child)
            .unwrap();
        let nested = private_child.join("input.json");
        fs::write(&nested, &bytes).unwrap();
        fs::set_permissions(&nested, Permissions::from_mode(0o600)).unwrap();
        for mode in [0o755, 0o770, 0o777] {
            fs::set_permissions(&private_child, Permissions::from_mode(mode)).unwrap();
            outputs.push(fixture.run(&fixture.arguments(&nested), false));
        }
        fs::set_permissions(&private_child, Permissions::from_mode(0o700)).unwrap();
        let parent_link = fixture.path("linked-parent");
        symlink(&private_child, &parent_link).unwrap();
        outputs.push(fixture.run(&fixture.arguments(&parent_link.join("input.json")), false));
        let mut unchanged = fs::read(&long).unwrap() == vec![b' '; 1024 * 1024]
            && fs::read(&nested).unwrap() == bytes;
        for path in regular {
            // Restore only fixture-owned file permissions after the child checks, so
            // zero-permission input can be read back without acquiring parent authority.
            fs::set_permissions(&path, Permissions::from_mode(0o600)).unwrap();
            unchanged &= fs::read(&path).unwrap() == bytes;
        }
        (outputs, unchanged)
    });
    assert!(unchanged);
    let generic = &outputs[0].output.stderr;
    for output in &outputs {
        rejected(output, 1);
        assert_eq!(&output.output.stderr, generic);
    }
}
