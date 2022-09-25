package main

import (
	"flag"
	"io"
	"log"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"

	"github.com/evanw/esbuild/pkg/api"
)

func cp() error {
	curFav, _ := os.Open("favicon.ico")
	curIdx, _ := os.Open("index.html")
	fav, _ := os.Create("dist/favicon.ico")
	idx, _ := os.Create("dist/index.html")
	if _, err := io.Copy(fav, curFav); err != nil {
		return err
	}
	if _, err := io.Copy(idx, curIdx); err != nil {
		return err
	}

	return nil
}

func main() {
	build := flag.Bool("build", false, "build")
	serve := flag.Bool("serve", false, "serve")
	flag.Parse()

	opts := api.BuildOptions{
		EntryPoints:       []string{"index.tsx"},
		Outfile:           "dist/index.js",
		Write:             true,
		MinifySyntax:      true,
		MinifyWhitespace:  true,
		MinifyIdentifiers: true,
		Bundle:            true,
		Sourcemap:         api.SourceMapLinked,
		Target:            api.ESNext,
		Engines: []api.Engine{
			{Name: api.EngineChrome, Version: "58"},
			{Name: api.EngineEdge, Version: "18"},
			{Name: api.EngineFirefox, Version: "57"},
			{Name: api.EngineNode, Version: "12"},
			{Name: api.EngineSafari, Version: "11"},
		},
	}

	if *serve {
		opts.Watch = &api.WatchMode{
			OnRebuild: func(result api.BuildResult) {
				if len(result.Errors) > 0 {
					for _, err := range result.Errors {
						log.Println(err)
					}
				} else {
					log.Println("new build succeeded")
					os.Link("index.html", "dist/index.html")
					os.Link("favicon.ico", "dist/favicon.ico")
				}
			},
		}
	}

	done := make(chan bool)
	go func() {
		result := api.Build(opts)

		if len(result.Errors) > 0 {
			log.Fatal(result.Errors)
		}

		if *serve {
			log.Println("watching for changes to source files")
			<-done
		}
		done <- true
	}()

	if *serve {
		u, err := url.Parse("http://[::]:8080")
		if err != nil {
			log.Fatal(err)
		}

		rp := httputil.NewSingleHostReverseProxy(u)
		http.Handle("/api/", rp)
		http.Handle("/", http.FileServer(http.Dir("dist")))
		log.Println("serving files from ./dist")
		log.Println("proxying requests for /api/* to [::]:8080")
		if err := http.ListenAndServe("[::]:8000", nil); err != nil {
			log.Println(err)
		}
		done <- true
	}
	if *build {
		<-done
		if err := cp(); err != nil {
			log.Fatal(err)
		}
	}
}
