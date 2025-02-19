.PHONY: gen clean

# The .proto file is in orchestrator/protos/pb/v1/embedding_service.proto
# but we reference it as pb/v1/embedding_service.proto relative to the -I path
PROTO_FILE          = v1/embedding_service.proto
PROTO_GO_OUT_DIR    = ./orchestrator/gen/pb
PROTO_PYTHON_OUT_DIR= ./embedding_service/gen/pb

gen:
	python -m grpc_tools.protoc \
		-I orchestrator/protos/pb \
		--python_out=$(PROTO_PYTHON_OUT_DIR) \
		--grpc_python_out=$(PROTO_PYTHON_OUT_DIR) \
		$(PROTO_FILE)

	protoc \
		-I orchestrator/protos/pb \
		--go_out=$(PROTO_GO_OUT_DIR) --go_opt=paths=source_relative \
		--go-grpc_out=$(PROTO_GO_OUT_DIR) --go-grpc_opt=paths=source_relative \
		$(PROTO_FILE)
