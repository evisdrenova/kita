package main

import (
	"database/sql"
	"fmt"
	"strings"
	"time"
)

// helper function for retrying database operations
func (fp *FileProcessor) retryDBOperation(operation func(*sql.Tx) error) error {
	maxRetries := 3
	for i := 0; i < maxRetries; i++ {
		tx, err := fp.Db.Begin()
		if err != nil {
			continue
		}

		err = operation(tx)
		if err != nil {
			tx.Rollback()
			if strings.Contains(err.Error(), "database is locked") {
				// Wait before retrying, with exponential backoff
				time.Sleep(time.Duration(i+1) * 500 * time.Millisecond)
				continue
			}
			return err
		}

		err = tx.Commit()
		if err != nil {
			if strings.Contains(err.Error(), "database is locked") {
				time.Sleep(time.Duration(i+1) * 500 * time.Millisecond)
				continue
			}
			return err
		}

		return nil
	}
	return fmt.Errorf("database is locked after %d retries", maxRetries)
}
