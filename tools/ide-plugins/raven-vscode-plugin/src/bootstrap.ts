import * as vscode from "vscode";
import * as os from "os";
import type { Config } from "./config";
import { log, isValidExecutable } from "./util";
import type { PersistentState } from "./persistent_state";
import { exec } from "child_process";

export async function bootstrap(
    context: vscode.ExtensionContext,
    config: Config,
    state: PersistentState,
): Promise<string> {
    const path = await getServer(context, config, state);
    if (!path) {
        throw new Error(
            "Raven Language Server is not available. " +
                "Please, ensure its [proper installation](https://rust-analyzer.github.io/manual.html#installation).",
        );
    }

    log.info("Using server binary at", path);

    if (!isValidExecutable(path, config.serverExtraEnv)) {
        if (config.serverPath) {
            throw new Error(`Failed to execute ${path} --version. \`config.server.path\` or \`config.serverPath\` has been set explicitly.\
            Consider removing this config or making a valid server binary available at that path.`);
        } else {
            throw new Error(`Failed to execute ${path} --version`);
        }
    }

    return path;
}
async function getServer(
    context: vscode.ExtensionContext,
    config: Config,
    state: PersistentState,
): Promise<string | undefined> {
    const explicitPath = process.env["__RV_LSP_SERVER_DEBUG"] ?? config.serverPath;
    if (explicitPath) {
        if (explicitPath.startsWith("~/")) {
            return os.homedir() + explicitPath.slice("~".length);
        }
        return explicitPath;
    }
    if (config.package.releaseTag === null) return "raven-analyzer";

    const ext = process.platform === "win32" ? ".exe" : "";
    const bundled = vscode.Uri.joinPath(context.extensionUri, "server", `raven-analyzer${ext}`);
    const bundledExists = await vscode.workspace.fs.stat(bundled).then(
        () => true,
        () => false,
    );
    if (bundledExists) {
        let server = bundled;
        await state.updateServerVersion(config.package.version);
        return server.fsPath;
    }

    await state.updateServerVersion(undefined);
    await vscode.window.showErrorMessage(
        "Unfortunately we don't ship binaries for your platform yet. " +
            "You need to manually clone the rust-analyzer repository and " +
            "run `cargo xtask install --server` to build the language server from sources. " +
            "If you feel that your platform should be supported, please create an issue " +
            "about that [here](https://github.com/rust-lang/rust-analyzer/issues) and we " +
            "will consider it.",
    );
    return undefined;
}