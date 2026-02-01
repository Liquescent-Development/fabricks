package main

import (
	"fmt"
	"net/http"
)

func main() {
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintln(w, "Hello from Fabricks! (Go/TinyGo HTTP)")
	})

	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		fmt.Fprintln(w, "OK")
	})

	// In WASI HTTP, ListenAndServe binds to the wasi:http incoming handler
	http.ListenAndServe(":8080", nil)
}
