/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import * as path from 'path';
import { ExtensionContext, Uri, workspace } from 'vscode';

import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	Executable,
	TransportKind
} from 'vscode-languageclient';

let client: LanguageClient;

export async function activate(context: ExtensionContext) {
	// The server is implemented in Rust
	const serverModule = context.asAbsolutePath(
		path.join('server', 'raven-language-server')
	);
	// If the extension is launched in debug mode then the debug server options are used
	// Otherwise the run options are used
	const serverOptions: ServerOptions = {
		command: serverModule,
		args: [],
		options: {
			shell: true
		}
	};

	// Options to control the language client
	const clientOptions: LanguageClientOptions = {
		// Register the server for plain text documents
		documentSelector: [{ scheme: 'file', language: 'raven' }]
	};

	// Create the language client and start the client.
	client = new LanguageClient(
		'ravenServer',
		'Raven Server',
		serverOptions,
		clientOptions
	);

	// Start the client. This will also launch the server
	client.start();
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
