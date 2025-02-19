.PHONY: gen

gen:
	python -m grpc_tools.protoc -I. \
		--python_out=./embedding_service/gen/pb \
		--grpc_python_out=./embedding_service/gen/pb \
		./orchestrator/protos/embedding_service.proto

	protoc -I. \
		--go_out=./orchestrator/gen/pb --go_opt=paths=source_relative \
		--go-grpc_out=./orchestrator/gen/pb --go-grpc_opt=paths=source_relative \
		./orchestrator/protos/embedding_service.proto