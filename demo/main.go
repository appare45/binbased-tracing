package main

import (
	_ "embed"
	"fmt"
	"log"
	"net/http"
	"sync"
)

//go:embed index.html
var indexHTML []byte

type Item struct {
	ID    int    `json:"id"`
	Value string `json:"value"`
}

type CreateItemRequest struct {
	Value string `json:"value"`
}

type Store struct {
	mu     sync.Mutex
	items  map[int]Item
	nextID int
}

func newStore() *Store {
	s := &Store{
		items:  make(map[int]Item),
		nextID: 1,
	}
	fruits := []string{
		"りんご", "みかん", "ぶどう", "もも", "なし",
		"すいか", "メロン", "いちご", "バナナ", "キウイ",
		"パイナップル", "マンゴー", "さくらんぼ", "レモン", "グレープフルーツ",
		"パパイヤ", "ライチ", "ドラゴンフルーツ", "ブルーベリー", "ラズベリー",
	}
	for i := range 100 {
		s.create(fmt.Sprintf("%s%d", fruits[i%len(fruits)], i/len(fruits)+1))
	}
	return s
}

type Server struct {
	store *Store
}

func main() {
	server := &Server{store: newStore()}

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			w.Header().Set("Content-Type", "text/html; charset=utf-8")
			w.Write(indexHTML)
		} else {
			http.NotFound(w, r)
		}
	})
	http.HandleFunc("/items", server.handleItems)
	http.HandleFunc("/items/", server.handleGetItem)
	http.HandleFunc("/health", server.handleHealth)

	log.Println("Starting server on :8080")
	log.Println("Open http://localhost:8080 in your browser")
	log.Fatal(http.ListenAndServe(":8080", nil))
}
