//! XHUP Flow 产品管理器无头 CLI(部署/验证用)。
//!
//! 复用桌面应用「控制中心」的同一套纯逻辑层(`manager`):detect →
//! inspect → plan → execute → redeploy。用于:
//!
//! - 真机部署验收(与 GUI 相同的信任边界:只写 OWNED_FILES、
//!   备份只进 xhup_backup/、绝不触碰用户学习数据);
//! - CI 中的端到端部署测试。
//!
//! 用法:
//!   cargo run -p trainer --example product_cli -- status [user_data_dir]
//!   cargo run -p trainer --example product_cli -- plan [user_data_dir]
//!   cargo run -p trainer --example product_cli -- install [user_data_dir]
//!   cargo run -p trainer --example product_cli -- uninstall [user_data_dir]
//!   cargo run -p trainer --example product_cli -- redeploy [user_data_dir]
//!
//! install/update/repair 共用同一计划(缺失 = Write,已存在 = 备份后
//! Overwrite);幂等,重复执行安全。

use std::path::PathBuf;
use std::process::ExitCode;

use trainer_lib::manager;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else {
        eprintln!("用法: product_cli <status|plan|install|uninstall|redeploy> [user_data_dir]");
        return ExitCode::from(2);
    };

    let user_data_dir: Option<PathBuf> = args.get(1).map(PathBuf::from);
    let (client, detected_dir) = manager::detect_platform();
    let dir = match user_data_dir.or_else(|| detected_dir.clone()) {
        Some(dir) => dir,
        None => {
            eprintln!("无法检测 Rime 用户数据目录,请显式传入");
            return ExitCode::FAILURE;
        }
    };

    let result = match command.as_str() {
        "status" => cmd_status(&client, &dir),
        "plan" => cmd_plan(&dir),
        "install" => cmd_execute(&dir),
        "uninstall" => cmd_uninstall(&dir),
        "redeploy" => cmd_redeploy(client),
        other => {
            eprintln!("未知命令: {other}");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("错误: {error}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_status(
    client: &manager::RimeClient,
    dir: &std::path::Path,
) -> Result<(), manager::ManagerError> {
    println!("客户端: {client:?}");
    println!("用户数据目录: {}", dir.display());
    let package = manager::RimePackage::bundled()?;
    println!("随附包版本: {}", package.version);
    let status = manager::install_status(dir, *client, Some(&package));
    let health = status.health(&package.version);
    println!(
        "健康度: {:?}(已安装 {}/{} 个文件,版本 {:?})",
        health, status.installed_files, status.total_files, status.installed_version
    );
    println!("已安装方案: {:?}", status.schemas);
    for (file, integrity) in package.files.iter().zip(&status.integrity) {
        println!("  {integrity:?} {file}", file = file.0);
    }
    let learning = manager::learning_summary(dir);
    println!(
        "学习数据: db={} 快照={} 工具={}",
        learning.db_exists, learning.snapshot_available, learning.tool_available
    );
    Ok(())
}

fn cmd_plan(dir: &std::path::Path) -> Result<(), manager::ManagerError> {
    let package = manager::RimePackage::bundled()?;
    let plan = manager::plan_install(dir, &package)?;
    println!("随附包版本: {}", package.version);
    println!("计划动作 {} 项:", plan.actions.len());
    for action in &plan.actions {
        match action {
            manager::PlanAction::Write { file } => println!("  新建    {file}"),
            manager::PlanAction::Overwrite { file, backup } => {
                println!("  替换    {file}(备份: {})", backup.display())
            }
            manager::PlanAction::Delete { file } => println!("  删除    {file}"),
        }
    }
    Ok(())
}

fn cmd_execute(dir: &std::path::Path) -> Result<(), manager::ManagerError> {
    let package = manager::RimePackage::bundled()?;
    let plan = manager::plan_install(dir, &package)?;
    let done = manager::execute(&plan, dir, Some(&package))?;
    println!("已执行 {done} 项;重跑 status 可验证健康度。");
    Ok(())
}

fn cmd_uninstall(dir: &std::path::Path) -> Result<(), manager::ManagerError> {
    let plan = manager::plan_uninstall(dir)?;
    let done = manager::execute(&plan, dir, None)?;
    println!("已删除 {done} 个 XHUP 拥有文件(学习数据与其他文件不受影响)。");
    Ok(())
}

fn cmd_redeploy(client: manager::RimeClient) -> Result<(), manager::ManagerError> {
    match client.redeploy_support() {
        manager::RedeploySupport::Automatic { program, args } => {
            let output = std::process::Command::new(&program)
                .args(&args)
                .output()
                .map_err(|source| {
                    eprintln!("无法启动 {}: {source}", program.display());
                    std::process::exit(1);
                })?;
            if !output.status.success() {
                eprintln!(
                    "{} 执行失败:{}",
                    program.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                );
                std::process::exit(1);
            }
            println!("已重新部署: {} {:?}", program.display(), args);
            Ok(())
        }
        manager::RedeploySupport::Manual => {
            println!("{}", client.redeploy_guidance());
            Ok(())
        }
    }
}
