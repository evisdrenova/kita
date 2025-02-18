package main

import (
	"os"
	"strings"
)

// gets the file category from the file extension
func getCategoryFromExtension(extension string) SearchCategory {
	if extension == "" {
		return CategoryOther
	}
	switch strings.ToLower(extension) {
	case ".app", ".exe", ".dmg":
		return CategoryApplications
	case ".pdf":
		return CategoryPDFDocuments
	case ".doc", ".docx", ".txt", ".rtf":
		return CategoryDocuments
	case ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp":
		return CategoryImages
	case ".js", ".ts", ".jsx", ".tsx", ".py", ".java", ".cpp",
		".html", ".css", ".json", ".xml", ".yaml", ".yml":
		return CategoryDocuments
	case ".mp4", ".mov", ".avi", ".mkv":
		return CategoryOther
	case ".mp3", ".wav", ".flac", ".m4a":
		return CategoryOther
	case ".xlsx", ".xls", ".csv":
		return CategorySpreadsheets
	case ".zip", ".rar", ".7z", ".tar", ".gz":
		return CategoryOther
	default:
		return CategoryOther
	}
}

// checks if the path is a directory
func isDirectory(path string) bool {
	info, err := os.Stat(path)
	if err != nil {
		return false
	}
	return info.IsDir()
}

func isPlainText(ext string) bool {
	plainTextExts := map[string]bool{
		".txt": true, ".js": true, ".ts": true,
		".jsx": true, ".tsx": true, ".py": true,
		".java": true, ".cpp": true, ".html": true,
		".css": true, ".json": true, ".xml": true,
		".yaml": true, ".yml": true,
	}
	return plainTextExts[ext]
}
