package main

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/ledongthuc/pdf"
	"github.com/unidoc/unioffice/document"
)

func (fp *FileProcessor) extractText(filePath string) (string, error) {
	ext := strings.ToLower(filepath.Ext(filePath))

	switch {
	case isPlainText(ext):
		return fp.extractTextFromPlain(filePath)
	case ext == ".pdf":
		return fp.extractTextFromPDF(filePath)
	case ext == ".docx":
		return fp.extractTextFromDOCX(filePath)
	default:
		return "", nil
	}
}

func (fp *FileProcessor) extractTextFromPlain(filePath string) (string, error) {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return "", err
	}
	return string(content), nil
}

func (fp *FileProcessor) extractTextFromPDF(filePath string) (string, error) {
	f, r, err := pdf.Open(filePath)
	if err != nil {
		return "", fmt.Errorf("malformed PDF: %v", err)
	}
	defer f.Close()

	var buf bytes.Buffer
	b, err := r.GetPlainText()
	if err != nil {
		return "", fmt.Errorf("malformed PDF: %v", err)
	}

	_, err = buf.ReadFrom(b)
	if err != nil {
		return "", fmt.Errorf("error reading PDF content: %v", err)
	}

	if buf.Len() == 0 {
		return "", fmt.Errorf("no text content found in PDF")
	}

	return buf.String(), nil
}

func (fp *FileProcessor) extractTextFromDOCX(filePath string) (string, error) {
	doc, err := document.Open(filePath)
	if err != nil {
		return "", err
	}
	var text strings.Builder
	for _, para := range doc.Paragraphs() {
		for _, run := range para.Runs() {
			text.WriteString(run.Text())
		}
		text.WriteString("\n")
	}
	return text.String(), nil
}
