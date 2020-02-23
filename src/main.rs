// Based on rnix-lsp (https://github.com/nix-community/rnix-lsp) with the
// following license:
//
// MIT License
// 
// Copyright (c) 2020 jD91mZM2
// 
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
// 
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
// 
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use log::{error, trace, warn};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    *,
    notification::{*, Notification as _},
    request::{*, Request as RequestTrait},
};
use std::{
    collections::HashMap,
    panic,
    process,
    rc::Rc,
};

type Error = Box<dyn std::error::Error>;

fn main() {
    if let Err(err) = real_main() {
        error!("Error: {} ({:?})", err, err);
        error!("A fatal error has occured and oclsp will shut down.");
        drop(err);
        process::exit(libc::EXIT_FAILURE);
    }
}
fn real_main() -> Result<(), Error> {
    env_logger::init();
    panic::set_hook(Box::new(move |panic| {
        error!("----- Panic -----");
        error!("{}", panic);
    }));

    let (connection, io_threads) = Connection::stdio();
    let capabilities = serde_json::to_value(&ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::Full),
                ..TextDocumentSyncOptions::default()
            }
        )),
        completion_provider: Some(CompletionOptions {
            ..CompletionOptions::default()
        }),
        definition_provider: Some(true),
        document_formatting_provider: Some(true),
        rename_provider: Some(RenameProviderCapability::Simple(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        ..ServerCapabilities::default()
    }).unwrap();

    connection.initialize(capabilities)?;

    App {
        files: HashMap::new(),
        conn: connection,
    }.main();

    io_threads.join()?;

    Ok(())
}

struct App {
    files: HashMap<Url, String>,
    conn: Connection,
}
impl App {
    fn reply(&mut self, response: Response) {
        trace!("Sending response: {:#?}", response);
        self.conn.sender.send(Message::Response(response)).unwrap();
    }
    fn notify(&mut self, notification: Notification) {
        trace!("Sending notification: {:#?}", notification);
        self.conn.sender.send(Message::Notification(notification)).unwrap();
    }
    fn err<E>(&mut self, id: RequestId, err: E)
        where E: std::fmt::Display
    {
        warn!("{}", err);
        self.reply(Response::new_err(id, ErrorCode::UnknownErrorCode as i32, err.to_string()));
    }
    fn main(&mut self) {
        while let Ok(msg) = self.conn.receiver.recv() {
            trace!("Message: {:#?}", msg);
            match msg {
                Message::Request(req) => {
                    let id = req.id.clone();
                    match self.conn.handle_shutdown(&req) {
                        Ok(true) => break,
                        Ok(false) => if let Err(err) = self.handle_request(req) {
                            self.err(id, err);
                        },
                        Err(err) => {
                            // This only fails if a shutdown was
                            // requested in the first place, so it
                            // should definitely break out of the
                            // loop.
                            self.err(id, err);
                            break;
                        },
                    }
                },
                Message::Notification(notification) => {
                    let _ = self.handle_notification(notification);
                },
                Message::Response(_) => (),
            }
        }
    }
    fn handle_request(&mut self, req: Request) -> Result<(), Error> {
        fn cast<Kind>(req: &mut Option<Request>) -> Option<(RequestId, Kind::Params)>
        where
            Kind: RequestTrait,
            Kind::Params: serde::de::DeserializeOwned,
        {
            match req.take().unwrap().extract::<Kind::Params>(Kind::METHOD) {
                Ok(value) => Some(value),
                Err(owned) => {
                    *req = Some(owned);
                    None
                },
            }
        }
        let mut req = Some(req);
        if let Some((id, params)) = cast::<GotoDefinition>(&mut req) {
            if let Some(pos) = self.lookup_definition(params) {
                self.reply(Response::new_ok(id, pos));
            } else {
                self.reply(Response::new_ok(id, ()));
            }
        } else if let Some((id, params)) = cast::<Completion>(&mut req) {
            let completions = self.completions(&params.text_document_position).unwrap_or_default();
            self.reply(Response::new_ok(id, completions));
        } else if let Some((id, params)) = cast::<Rename>(&mut req) {
            let changes = self.rename(params);
            self.reply(Response::new_ok(id, WorkspaceEdit {
                changes,
                ..WorkspaceEdit::default()
            }));
        } else if let Some((id, params)) = cast::<Formatting>(&mut req) {
            let changes: Vec<TextEdit> = if let Some(code) = self.files.get(&params.text_document.uri) {
                unimplemented!()
                //let fmt = nixpkgs_fmt::reformat_node(&ast.node());
                //fmt.text_diff().iter()
                //    .filter(|range| !range.delete.is_empty() || !range.insert.is_empty())
                //    .map(|edit| TextEdit {
                //        range: utils::range(&code, edit.delete),
                //        new_text: edit.insert.to_string()
                //    })
                //    .collect()
            } else {
                Vec::new()
            };
            self.reply(Response::new_ok(id, changes));
        } else if let Some((id, params)) = cast::<SelectionRangeRequest>(&mut req) {
            let mut selections: Vec<Option<SelectionRange>> = Vec::new();
            if let Some(code) = self.files.get(&params.text_document.uri) {
                for pos in params.positions {
                    // TODO
                    //selections.push(utils::selection_ranges(&ast.node(), code, pos));
                }
            }
            self.reply(Response::new_ok(id, selections));
        }
        Ok(())
    }
    fn handle_notification(&mut self, req: Notification) -> Result<(), Error> {
        match &*req.method {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
                let text = params.text_document.text;
                // TODO
                //let parsed = rnix::parse(&text);
                //self.send_diagnostics(params.text_document.uri.clone(), &text, &parsed)?;
                self.files.insert(params.text_document.uri, text);
            },
            DidChangeTextDocument::METHOD => {
                let params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
                if let Some(change) = params.content_changes.into_iter().last() {
                    // TODO
                    //let parsed = rnix::parse(&change.text);
                    //self.send_diagnostics(params.text_document.uri.clone(), &change.text, &parsed)?;
                    self.files.insert(params.text_document.uri, change.text);
                }
            },
            _ => (),
        }
        Ok(())
    }
    fn lookup_definition(&mut self, params: TextDocumentPositionParams) -> Option<Location> {
        let current_content = self.files.get(&params.text_document.uri)?;
        // TODO
        unimplemented!();
        //let offset = utils::lookup_pos(current_content, params.position)?;
        //let node = current_ast.node();
        //let (name, scope) = self.scope_for_ident(params.text_document.uri, &node, offset)?;

        //let var = scope.get(name.as_str())?;
        //let (_definition_ast, definition_content) = self.files.get(&var.file)?;
        //Some(Location {
        //    uri: (*var.file).clone(),
        //    range: utils::range(definition_content, var.key.text_range())
        //})
    }
    #[allow(clippy::shadow_unrelated)] // false positive
    fn completions(&mut self, params: &TextDocumentPositionParams) -> Option<Vec<CompletionItem>> {
        let content = self.files.get(&params.text_document.uri)?;
        // TODO
        None
        //let offset = utils::lookup_pos(content, params.position)?;

        //let node = ast.node();
        //let (name, scope) = self.scope_for_ident(params.text_document.uri.clone(), &node, offset)?;

        //// Re-open, because scope_for_ident may mutably borrow
        //let (_, content) = self.files.get(&params.text_document.uri)?;

        //let mut completions = Vec::new();
        //for var in scope.keys() {
        //    if var.starts_with(&name.as_str()) {
        //        completions.push(CompletionItem {
        //            label: var.clone(),
        //            text_edit: Some(TextEdit {
        //                range: utils::range(content, name.node().text_range()),
        //                new_text: var.clone()
        //            }),
        //            ..CompletionItem::default()
        //        });
        //    }
        //}
        //Some(completions)
    }
    fn rename(&mut self, params: RenameParams) -> Option<HashMap<Url, Vec<TextEdit>>> {
        struct Rename<'a> {
            edits: Vec<TextEdit>,
            code: &'a str,
            old: &'a str,
            new_name: String,
        }

        let uri = params.text_document_position.text_document.uri;
        let code = self.files.get(&uri)?;
        // TODO
        None
    }
    fn send_diagnostics(&mut self, uri: Url, code: &str) -> Result<(), Error> {
        // TODO
        unimplemented!()
        //let errors = ast.errors();
        //let mut diagnostics = Vec::with_capacity(errors.len());
        //for err in errors {
        //    if let ParseError::Unexpected(node) = err {
        //        diagnostics.push(Diagnostic {
        //            range: utils::range(code, node),
        //            severity: Some(DiagnosticSeverity::Error),
        //            message: err.to_string(),
        //            ..Diagnostic::default()
        //        });
        //    }
        //}
        //self.notify(Notification::new(
        //    "textDocument/publishDiagnostics".into(),
        //    PublishDiagnosticsParams {
        //        uri,
        //        diagnostics,
        //        version: None,
        //    }
        //));
        //Ok(())
    }
}
