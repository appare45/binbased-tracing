package main

import (
	"encoding/json"
	"log"
	"net/http"
	"strconv"
	"strings"
)

func respondJSON(w http.ResponseWriter, status int, data any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(data); err != nil {
		log.Printf("Error encoding JSON: %v", err)
	}
}

func respondError(w http.ResponseWriter, status int, message string) {
	respondJSON(w, status, map[string]string{"error": message})
}

func (s *Server) handleItems(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodPost:
		s.handleCreateItem(w, r)
	case http.MethodGet:
		s.handleGetAllItems(w, r)
	default:
		respondError(w, http.StatusMethodNotAllowed, "Method not allowed")
	}
}

func (s *Server) handleCreateItem(w http.ResponseWriter, r *http.Request) {
	var req CreateItemRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		respondError(w, http.StatusBadRequest, "Invalid JSON")
		return
	}

	if strings.TrimSpace(req.Value) == "" {
		respondError(w, http.StatusBadRequest, "value cannot be empty")
		return
	}

	item := s.store.create(req.Value)
	log.Printf("Created item: id=%d, value=%s", item.ID, item.Value)
	respondJSON(w, http.StatusCreated, item)
}

func (s *Server) handleGetAllItems(w http.ResponseWriter, r *http.Request) {
	ids := s.store.listIDs()
	respondJSON(w, http.StatusOK, ids)
}

func (s *Server) handleGetItem(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		respondError(w, http.StatusMethodNotAllowed, "Method not allowed")
		return
	}

	idStr := strings.TrimPrefix(r.URL.Path, "/items/")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		respondError(w, http.StatusBadRequest, "Invalid ID format")
		return
	}

	item, ok := s.store.get(id)
	if !ok {
		respondError(w, http.StatusNotFound, "Item not found")
		return
	}

	respondJSON(w, http.StatusOK, item)
}

func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		respondError(w, http.StatusMethodNotAllowed, "Method not allowed")
		return
	}
	respondJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}
