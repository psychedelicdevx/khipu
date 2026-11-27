import http from "node:http";

const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  if (url.pathname === "/hello") {
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("hello");
  } else if (url.pathname === "/json") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ ok: true, path: url.pathname, query: Object.fromEntries(url.searchParams) }));
  } else if (url.pathname === "/file") {
    res.writeHead(200, { "content-type": "application/octet-stream" });
    res.end("hello khipu\n");
  } else {
    res.writeHead(404);
    res.end("not found");
  }
});

server.listen(Number(process.env.PORT ?? 8080), "127.0.0.1");
