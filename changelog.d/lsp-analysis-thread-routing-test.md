<!-- section: Changed -->
- LSP tests now fail if refreshes stop running on the one long-lived
  `ripr-lsp-analysis` thread, which keeps the server's memory in one malloc
  arena. The analysis thread's restart path and panic message are also
  covered (#6632).
