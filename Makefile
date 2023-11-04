NAME=min_review_bot
VERSION=$(shell tq -f Cargo.toml .package.version | tr -d '"')
USER=rdelfin
TAG=${USER}/${NAME}:${VERSION}

.PHONY: build
build:
	docker build . -t ${TAG}

.PHONY: publish
publish: build
	docker push ${TAG}
