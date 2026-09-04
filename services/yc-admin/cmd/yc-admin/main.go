package main

import (
	"log"
	"net/http"
	"os"
	"path/filepath"

	"yc-admin/internal/api"
	"yc-admin/internal/config"
	"yc-admin/internal/service"
	"yc-admin/internal/store"
)

func main() {
	cfg := config.Load()
	if err := os.MkdirAll(cfg.DataDir, 0o755); err != nil {
		log.Fatal(err)
	}

	// Prefer serving web UI relative to module root when launched from elsewhere.
	if _, err := os.Stat("web"); err != nil {
		if exe, e := os.Executable(); e == nil {
			cand := filepath.Join(filepath.Dir(exe), "web")
			if _, err2 := os.Stat(cand); err2 == nil {
				_ = os.Chdir(filepath.Dir(exe))
			}
		}
	}

	st, err := store.Open(cfg.DataDir)
	if err != nil {
		log.Fatal(err)
	}
	defer st.Close()

	svc := &service.Services{Store: st, PublicBase: cfg.PublicBase}
	srv := api.New(cfg, svc, st)

	log.Printf("yc-admin listening on %s (data=%s)", cfg.Addr, cfg.DataDir)
	log.Printf("admin token header: X-Admin-Token (default from YC_ADMIN_TOKEN)")
	if err := http.ListenAndServe(cfg.Addr, srv.Handler()); err != nil {
		log.Fatal(err)
	}
}
