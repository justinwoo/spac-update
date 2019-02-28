#![warn(rust_2018_idioms, clippy::all)]

use rayon::prelude::*;
use regex::Captures;
use regex::RegexBuilder;
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::Command;

// unquoted package name
struct PkgName(pub String);

// parameters for a given package
struct PkgParams {
    name: String,
    dependencies: String,
    url: String,
    version: String,
}

// package expression for Spacchetti
struct PkgExpr(pub String);

fn main() {
    let args: Vec<String> = env::args().collect();

    let cmd = args
        .get(1)
        .unwrap_or_else(|| panic!("you must provide an action. {}", HELP_MSG));

    match cmd.as_ref() {
        "help" => help(),
        "from-bower" => {
            let package_name = as_package_name(args.get(2));
            from_bower(&package_name);
        }
        "prepare-bower" => prepare_bower(&as_package_name(args.get(2))),
        "update-all" => update_all(),
        _ => panic!(r#"unknown command "{}". see help."#, cmd),
    }
}

fn update_all() {
    let deps = run_command("jq 'keys[]' packages.json -r")
        .lines()
        .map(|s| PkgName(s.to_string()))
        .collect::<Vec<_>>();

    deps.par_iter().map(prepare_bower).collect::<()>();
    println!("Finished prepare-bower");

    for dep in deps {
        from_bower(&dep)
    }
    println!("Finished from-bower");
}

fn from_bower(pkg_name: &PkgName) {
    let pkg_params = prepare_pkg_params(&pkg_name);
    let PkgExpr(expr) = prepare_pkg_expr(&pkg_params);

    // figure out what the group name should be, and quit if group name can't be determined.
    let url_splits: Vec<&str> = pkg_params.url.split('/').collect();
    if url_splits.len() != 5 {
        return println!(
            r#"Could not match group name in url "{}" for package "{}""#,
            pkg_params.url, pkg_params.name
        );
    }
    let group_name = url_splits[3];

    // file to write our expression to, which may or may not exist already
    let file_name = format!("src/groups/{}.dhall", group_name);

    // check if package already exists
    let check = run_command(&format!(r#"jq '."{}"?' packages.json"#, pkg_params.name));

    if check == "null" {
        // the package does not exist in the set, and the group file may or may not exist already
        if Path::new(&file_name).exists() {
            // update an existing file and append
            let contents = read_file(&file_name);
            let last = contents
                .rfind('}')
                .unwrap_or_else(|| panic!("invalid dhall expression parsed from group file."));
            let (head, _) = contents.split_at(last);
            write_file_for_pkg(&file_name, &pkg_name, &format!("{},{}}}", head, expr));
        } else {
            // write a new file
            write_file_for_pkg(
                &file_name,
                &pkg_name,
                &format!("let mkPackage = ./../mkPackage.dhall in {{{}}}", expr),
            );
        }
    } else {
        // if the package already exists in the set, then we should go update the already existing group file
        let contents = read_file(&file_name);

        // match on the contents of the in file and replace the section that matches our package and write to the file
        let pkg_expr_regex = RegexBuilder::new(&format!(
            r#"( {} =\s*mkPackage\s*\[[^\]]*\][^"]*"[^\s]*"\s*"[^\s]*")"#,
            pkg_params.name
        ))
        .dot_matches_new_line(true)
        .build()
        .unwrap();

        let replaced = pkg_expr_regex
            .replace(&contents, |_: &Captures<'_>| expr.to_string())
            .to_string();
        write_file_for_pkg(&file_name, &pkg_name, &replaced);
    }
}

fn read_file(path: &str) -> String {
    let mut contents: String = String::new();
    File::open(&path)
        .unwrap_or_else(|_| panic!("could not open file: {}", &path))
        .read_to_string(&mut contents)
        .unwrap_or_else(|_| panic!("Could not extract contents of file {}", &path));
    contents
}

fn write_file_for_pkg(path: &str, pkg_name: &PkgName, contents: &str) {
    File::create(&path)
        .unwrap_or_else(|_| panic!("could not open group file {}", &path))
        .write_fmt(format_args!("{}", contents))
        .unwrap_or_else(|_| panic!("Unable to write to file {}", path));
    println!("updated expression for package {}", pkg_name.0);
}

fn prepare_bower(pkg_name: &PkgName) {
    let PkgExpr(expr) = prepare_pkg_expr(&prepare_pkg_params(pkg_name));
    println!("{}", expr);
}

fn run_command(command: &str) -> String {
    let command_ref = &command;
    let attempt = Command::new("bash")
        .arg("-c")
        .arg(command_ref)
        .output()
        .expect("Failed to launch bash command");

    if attempt.status.success() {
        let result: String = String::from_utf8(attempt.stdout)
            .unwrap_or_else(|_| panic!("Invalid output from command {}", command));
        result.trim().to_string()
    } else {
        panic!("Command failed: {}", command)
    }
}

fn prepare_pkg_expr(pkg_params: &PkgParams) -> PkgExpr {
    // prepare the pkg dhall expr that might be used in other places
    let expr = format!(
        "{} = mkPackage\n{}\n\"{}.git\"\n\"v{}\"",
        pkg_params.name, pkg_params.dependencies, pkg_params.url, pkg_params.version
    );
    PkgExpr(expr)
}

fn prepare_pkg_params(pkg_name: &PkgName) -> PkgParams {
    let PkgName(name) = pkg_name;

    // ensure bower-info dir
    run_command("mkdir -p bower-info");

    // filepath where we will store the bower info json
    let filepath = format!("bower-info/{}.json", name);

    // get the bower info if we dont have it
    run_command(&format!(
        "! test -f {} && bower info purescript-{} --json > {} || exit 0",
        filepath, name, filepath
    ));

    // check if the result was empty or not
    run_command(&format!("test -s {} || (echo 'Bower info for purescript-{} was empty. Does this library exist on Bower?' && exit 1)", filepath, name));

    // deps, or fall through to empty list
    let dependencies = run_command(&format!(
        r#"jq '.latest.dependencies // [] | keys | map(.[11:])' {}"#,
        filepath
    ));

    // url needs to remove some crud so we have a proper https url
    let url = run_command(&format!(r#"jq '.latest.repository.url // .latest.homepage' {} -r | sed -e "s/git:/https:/g" -e "s/com:/com\//g" -e "s/git@/https:\/\//" -e "s/\.git//g""#, filepath));

    // version, but stripping "v" that bower is sadly inconsistent on
    let version = run_command(&format!(
        r#"jq '.latest.version' {} -r | sed 's/v//g'"#,
        filepath
    ));

    PkgParams {
        name: name.to_string(),
        dependencies,
        url,
        version,
    }
}

fn as_package_name(arg: Option<&String>) -> PkgName {
    let string = arg
        .unwrap_or_else(|| panic!("you must provide a package name for this command. see help."));
    PkgName(string.to_string())
}

fn help() {
    println!("{}", HELP_MSG);
}

const HELP_MSG: &str = "
spac-update: update spacchetti

commands:
  from-bower [pkgname]
    update a single package from bower
  prepare-bower [pkgname]
    prepare the bower information by downloading the bower information
  update-all
    update all packages in packages.json through bower
  help
    get this help message
";
