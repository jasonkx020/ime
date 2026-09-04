package api

import (
	"encoding/json"
	"io"
	"net/http"
	"strconv"

	"yc-admin/internal/config"
	"yc-admin/internal/model"
	"yc-admin/internal/service"
	"yc-admin/internal/store"
)

type Server struct {
	cfg config.Config
	svc *service.Services
	st  *store.Store
	mux *http.ServeMux
}

func New(cfg config.Config, svc *service.Services, st *store.Store) *Server {
	s := &Server{cfg: cfg, svc: svc, st: st, mux: http.NewServeMux()}
	s.routes()
	return s
}

func (s *Server) Handler() http.Handler {
	return s.withCORS(s.mux)
}

func (s *Server) routes() {
	s.mux.HandleFunc("GET /healthz", s.handleHealth)
	s.mux.HandleFunc("GET /api/v1/dashboard", s.admin(s.handleDashboard))

	s.mux.HandleFunc("GET /api/v1/langpacks", s.admin(s.handleListPacks))
	s.mux.HandleFunc("POST /api/v1/langpacks", s.admin(s.handleCreatePack))
	s.mux.HandleFunc("GET /api/v1/langpacks/{id}", s.admin(s.handleGetPack))
	s.mux.HandleFunc("PATCH /api/v1/langpacks/{id}", s.admin(s.handlePatchPack))
	s.mux.HandleFunc("POST /api/v1/langpacks/{id}/upload", s.admin(s.handleUploadPack))
	s.mux.HandleFunc("POST /api/v1/langpacks/{id}/publish", s.admin(s.handlePublishPack))
	s.mux.HandleFunc("POST /api/v1/langpacks/{id}/archive", s.admin(s.handleArchivePack))

	s.mux.HandleFunc("GET /api/v1/catalog", s.handleCatalog)
	s.mux.HandleFunc("GET /cdn/langpacks/{file}", s.handleCDN)

	s.mux.HandleFunc("POST /api/v1/habits/events", s.handleIngestHabits)
	s.mux.HandleFunc("GET /api/v1/profiles", s.admin(s.handleListProfiles))
	s.mux.HandleFunc("GET /api/v1/profiles/{device_id}", s.admin(s.handleGetProfile))
	s.mux.HandleFunc("POST /api/v1/profiles/{device_id}/rebuild", s.admin(s.handleRebuildProfile))
	s.mux.HandleFunc("GET /api/v1/personalization/{device_id}", s.handlePersonalization)
	s.mux.HandleFunc("POST /api/v1/personalization/rebuild-all", s.admin(s.handleRebuildAll))

	fs := http.FileServer(http.Dir("web"))
	s.mux.Handle("GET /", fs)
	s.mux.Handle("GET /static/", http.StripPrefix("/static/", http.FileServer(http.Dir("web/static"))))
}

type handlerFunc func(http.ResponseWriter, *http.Request)

func (s *Server) admin(next handlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		token := r.Header.Get("X-Admin-Token")
		if token == "" {
			token = r.URL.Query().Get("token")
		}
		if token != s.cfg.AdminToken {
			writeErr(w, http.StatusUnauthorized, "unauthorized")
			return
		}
		next(w, r)
	}
}

func (s *Server) withCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, X-Admin-Token")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, OPTIONS")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (s *Server) handleHealth(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

func (s *Server) handleDashboard(w http.ResponseWriter, r *http.Request) {
	st, err := s.st.Dashboard(r.Context())
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, st)
}

func (s *Server) handleListPacks(w http.ResponseWriter, r *http.Request) {
	status := r.URL.Query().Get("status")
	list, err := s.st.ListLangPacks(r.Context(), status)
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"items": list})
}

func (s *Server) handleCreatePack(w http.ResponseWriter, r *http.Request) {
	var body model.LangPack
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeErr(w, http.StatusBadRequest, "invalid json")
		return
	}
	p, err := s.svc.CreateDraft(r.Context(), body)
	if err != nil {
		writeErr(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusCreated, p)
}

func (s *Server) handleGetPack(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeErr(w, http.StatusBadRequest, "bad id")
		return
	}
	p, err := s.st.GetLangPack(r.Context(), id)
	if err != nil {
		writeErr(w, http.StatusNotFound, "not found")
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handlePatchPack(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeErr(w, http.StatusBadRequest, "bad id")
		return
	}
	var body struct {
		DisplayName    string `json:"display_name"`
		MinHostVersion string `json:"min_host_version"`
		Notes          string `json:"notes"`
		Status         string `json:"status"`
	}
	if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
		writeErr(w, http.StatusBadRequest, "invalid json")
		return
	}
	p, err := s.svc.UpdateMeta(r.Context(), id, body.DisplayName, body.MinHostVersion, body.Notes, body.Status)
	if err != nil {
		writeErr(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handleUploadPack(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeErr(w, http.StatusBadRequest, "bad id")
		return
	}
	if err := r.ParseMultipartForm(64 << 20); err != nil {
		writeErr(w, http.StatusBadRequest, "multipart required")
		return
	}
	file, hdr, err := r.FormFile("file")
	if err != nil {
		writeErr(w, http.StatusBadRequest, "file field required")
		return
	}
	defer file.Close()
	p, err := s.svc.UploadArtifact(r.Context(), id, file, hdr.Filename)
	if err != nil {
		writeErr(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handlePublishPack(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeErr(w, http.StatusBadRequest, "bad id")
		return
	}
	p, err := s.svc.Publish(r.Context(), id)
	if err != nil {
		writeErr(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handleArchivePack(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.ParseInt(r.PathValue("id"), 10, 64)
	if err != nil {
		writeErr(w, http.StatusBadRequest, "bad id")
		return
	}
	p, err := s.svc.Archive(r.Context(), id)
	if err != nil {
		writeErr(w, http.StatusBadRequest, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handleCatalog(w http.ResponseWriter, r *http.Request) {
	cat, err := s.svc.BuildCatalog(r.Context())
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, cat)
}

func (s *Server) handleCDN(w http.ResponseWriter, r *http.Request) {
	path, err := s.svc.ArtifactPath(r.PathValue("file"))
	if err != nil {
		writeErr(w, http.StatusNotFound, "not found")
		return
	}
	http.ServeFile(w, r, path)
}

func (s *Server) handleIngestHabits(w http.ResponseWriter, r *http.Request) {
	var body struct {
		Events []model.HabitEvent `json:"events"`
	}
	dec := json.NewDecoder(io.LimitReader(r.Body, 4<<20))
	if err := dec.Decode(&body); err != nil {
		writeErr(w, http.StatusBadRequest, "invalid json")
		return
	}
	n, err := s.svc.IngestHabits(r.Context(), body.Events)
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"accepted": n})
}

func (s *Server) handleListProfiles(w http.ResponseWriter, r *http.Request) {
	ids, err := s.st.ListProfileIDs(r.Context(), 100)
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"device_ids": ids})
}

func (s *Server) handleGetProfile(w http.ResponseWriter, r *http.Request) {
	p, err := s.svc.GetProfile(r.Context(), r.PathValue("device_id"))
	if err != nil {
		writeErr(w, http.StatusNotFound, "not found")
		return
	}
	writeJSON(w, http.StatusOK, p)
}

func (s *Server) handleRebuildProfile(w http.ResponseWriter, r *http.Request) {
	pack, err := s.svc.RebuildProfileAndBoosts(r.Context(), r.PathValue("device_id"))
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, pack)
}

func (s *Server) handlePersonalization(w http.ResponseWriter, r *http.Request) {
	pack, err := s.svc.GetPersonalization(r.Context(), r.PathValue("device_id"))
	if err != nil {
		writeErr(w, http.StatusNotFound, "not found")
		return
	}
	writeJSON(w, http.StatusOK, pack)
}

func (s *Server) handleRebuildAll(w http.ResponseWriter, r *http.Request) {
	n, err := s.svc.RebuildAllActive(r.Context())
	if err != nil {
		writeErr(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"rebuilt": n})
}

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func writeErr(w http.ResponseWriter, code int, msg string) {
	writeJSON(w, code, map[string]string{"error": msg})
}
