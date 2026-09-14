package main

import (
	"errors"
	"fmt"
	"os"
	"time"

	oreslocks "github.com/ORESoftware/ores-locks-and-leases/src/go"
)

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run() error {
	if len(os.Args) < 4 {
		return errors.New("usage: helper <hold|contend|acquire> <path> <owner> [release-sentinel]")
	}
	mode, path, owner := os.Args[1], os.Args[2], os.Args[3]

	switch mode {
	case "hold":
		if len(os.Args) != 5 {
			return errors.New("hold requires release sentinel")
		}
		lock, acquired, err := oreslocks.TryAcquireLocalFileLock(path, owner)
		if err != nil {
			return err
		}
		if !acquired {
			return errors.New("holder unexpectedly contended")
		}
		fmt.Println("HELD")
		deadline := time.Now().Add(20 * time.Second)
		for {
			if _, err := os.Stat(os.Args[4]); err == nil {
				break
			} else if !errors.Is(err, os.ErrNotExist) {
				return fmt.Errorf("inspect release sentinel: %w", err)
			}
			if time.Now().After(deadline) {
				return errors.New("timed out waiting for release sentinel")
			}
			time.Sleep(5 * time.Millisecond)
		}
		if err := lock.Release(); err != nil {
			return err
		}
		fmt.Println("RELEASED")
	case "contend":
		lock, acquired, err := oreslocks.TryAcquireLocalFileLock(path, owner)
		if err != nil {
			return err
		}
		if acquired {
			_ = lock.Release()
			return errors.New("contender unexpectedly acquired live cross-runtime lock")
		}
		fmt.Println("CONTENDED")
	case "acquire":
		lock, acquired, err := oreslocks.TryAcquireLocalFileLock(path, owner)
		if err != nil {
			return err
		}
		if !acquired {
			return errors.New("post-release acquire unexpectedly contended")
		}
		fmt.Println("ACQUIRED")
		return lock.Release()
	default:
		return fmt.Errorf("unknown mode %q", mode)
	}
	return nil
}
