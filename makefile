.PHONY: gen clean

PROTO_GO_OUT_DIR    = ./orchestrator/gen
PROTO_TS_OUT_DIR    = ./gen/ts
PROTO_PYTHON_OUT_DIR= ./embedding_service/gen

clean:
	rm -rf $(PROTO_GO_OUT_DIR)/*
	rm -rf $(PROTO_TS_OUT_DIR)/*
	rm -rf $(PROTO_PYTHON_OUT_DIR)/*
	mkdir -p $(PROTO_GO_OUT_DIR)
	mkdir -p $(PROTO_TS_OUT_DIR)
	mkdir -p $(PROTO_PYTHON_OUT_DIR)

gen: clean
	buf generate