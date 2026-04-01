#!/usr/bin/env sh

set -eu

COMMAND="${1:-up}"

case "$COMMAND" in
  build)
    docker compose build
    ;;
  up)
    docker compose up --build -d
    ;;
  down)
    docker compose down
    ;;
  logs)
    docker compose logs -f --tail=200
    ;;
  status)
    docker compose ps
    ;;
  reset)
    docker compose down -v --remove-orphans
    ;;
  mysql)
    docker compose exec mysql mysql -uapp_user -papp_password_local hospital_platform
    ;;
  *)
    echo "Unknown command: $COMMAND"
    echo "Usage: ./scripts/stack.sh [build|up|down|logs|status|reset|mysql]"
    exit 1
    ;;
esac
