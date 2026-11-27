package main

import (
	"cmp"
	"encoding/json"
	"net/http"
	"os"
)

func main() {
	mux := http.NewServeMux()
	mux.HandleFunc("/hello", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte("hello"))
	})
	mux.HandleFunc("/json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{"ok": true, "path": r.URL.Path, "query": r.URL.Query()})
	})
	mux.HandleFunc("/file", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/octet-stream")
		w.Write([]byte("hello khipu\n"))
	})
	http.ListenAndServe("127.0.0.1:"+cmp.Or(os.Getenv("PORT"), "8080"), mux)
}
