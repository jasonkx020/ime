package config

import (
	"os"
	"strconv"
)

type Config struct {
	Addr       string
	DataDir    string
	AdminToken string
	PublicBase string
}

func Load() Config {
	return Config{
		Addr:       env("YC_ADMIN_ADDR", ":8080"),
		DataDir:    env("YC_ADMIN_DATA", "./data"),
		AdminToken: env("YC_ADMIN_TOKEN", "dev-token"),
		PublicBase: env("YC_ADMIN_PUBLIC_BASE", "http://127.0.0.1:8080"),
	}
}

func env(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func EnvInt(key string, def int) int {
	v := os.Getenv(key)
	if v == "" {
		return def
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return def
	}
	return n
}
