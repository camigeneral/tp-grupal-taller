# Taller de Programación - A Todo Rust

Editor de documentos colaborativo implementado en Rust.

## Integrantes

- Camila General  
- Ramiro Mantero  
- Valentina Moreno  
- Franco Secchi  

## Requisitos previos

Para compilar y ejecutar este proyecto, es necesario tener instalado:

- [Rust](https://www.rust-lang.org/tools/install)
- Cargo (incluido con Rust)
- Docker y Docker Compose
- Las siguientes dependencias del sistema para compilar con `Relm4` y `GTK4`:

```bash
sudo apt-get install -y \
    libglib2.0-dev \
    pkg-config \
    libpango1.0-dev \
    libgdk-pixbuf2.0-dev \
    libgtk-4-dev
```

## Arquitectura

El proyecto está estructurado en múltiples componentes que se comunican entre sí:

### Componentes

1. **Cliente (GUI)**
   - Interfaz gráfica usando GTK4
   - Permite edición de documentos
   - Envía comandos al microservicio
   - Recibe actualizaciones en tiempo real

2. **Microservicio**
   - Distribuye actualizaciones a los clientes suscritos
   - Persiste periódicamente el contenido de los documentos en Redis
   - Maneja múltiples nodos Redis

3. **Servidor Redis**
   - Almacena los documentos y su contenido
   - Maneja la persistencia de datos
   - Implementa el protocolo RESP
   - Gestiona las suscripciones

4. **Microservicio LLM**
   - Procesa solicitudes de lenguaje natural
   - Se conecta a la API de Gemini
   - Maneja múltiples conexiones concurrentes

## Estructura del Proyecto

```
├── client/                 # Cliente GUI
├── microservice/          # Microservicio de control
├── llm_microservice/     # Microservicio LLM
├── redis_server/         # Servidor Redis
├── rusty_docs/           # Biblioteca compartida
├── docker-compose.yml    # Configuración de Docker
├── Makefile              # Comandos de automatización
└── README.md
```

## Comandos Disponibles

El proyecto incluye un Makefile con comandos útiles para desarrollo y despliegue:

### Desarrollo Local

```bash
# Ejecutar el cliente en local
make client port=4000

# Construir todos los servicios
make build

# Construir un servicio específico
make build service=redis_server
```

### Gestión de Contenedores

```bash
# Levantar todos los servicios
make up

# Levantar un servicio específico
make up service=redis_server

# Ver logs de un servicio
make logs service=redis_server

# Detener todos los servicios
make stop

# Detener un servicio específico
make stop service=microservice

# Bajar y eliminar volúmenes
make down

# Reiniciar todos los servicios
make restart

# Reiniciar un servicio específico
make restart service=llm_microservice
```

### Utilidades

```bash
# Ver servicios corriendo
make ps

# Acceder a un contenedor
make exec service=redis_server

# Acceder con comando específico
make exec service=microservice cmd="cargo test"

# Eliminar contenedores parados
make rm

# Limpiar todo (containers, networks, volumes, images)
make prune

# Rebuild forzado (sin cache)
make rebuild

# Limpiar completamente
make clean
```

## Ejecución

### Opción 1: Con Docker (Recomendado)

1. **Levantar todos los servicios:**
```bash
make up
```

2. **Ver logs de un servicio específico:**
```bash
make logs service=microservice
```

3. **Acceder a un contenedor:**
```bash
make exec service=redis_server
```

### Opción 2: Desarrollo Local

1. **Servidor Redis:**
```bash
cargo run --bin redis_server 4000
```

2. **Microservicio:**
```bash
cd microservice && cargo run
```

3. **Microservicio LLM:**
```bash
cd llm_microservice && cargo run
```

4. **Cliente:**
```bash
make client port=4000
```

## Ejecución

### Forma segura recomendada para levantar el proyecto con Docker

Ejecuta estos comandos en orden para evitar problemas con imágenes o contenedores viejos:

```bash
make prune          # Limpia imágenes no usadas y elimina imágenes del proyecto
make build_images   # Construye la imagen base y las demás sin cache
make up             # Levanta todos los servicios en segundo plano


## Configuración

### Variables de Entorno

Para el modo Docker, configura las siguientes variables:

```bash
# API Key para Gemini (LLM) - REQUERIDA
export GEMINI_API_KEY="tu_api_key_aqui"

# Direcciones de los nodos Redis (opcional, ya configurado en docker-compose.yml)
export REDIS_NODE_HOSTS="node0:4000,node1:4001,node2:4002"
```

### Configuración de la API Key de Gemini

1. **Obtener una API Key:**
   - Ve a [Google AI Studio](https://makersuite.google.com/app/apikey)
   - Crea una nueva API key
   - Copia la clave generada

2. **Configurar la API Key:**

   **Opción A: Variable de entorno (Recomendado)**
   ```bash
   export GEMINI_API_KEY="tu_api_key_aqui"
   make up
   ```

   **Opción B: Archivo .env**
   ```bash
   # Copia el archivo de ejemplo
   cp env.example .env
   
   # Edita .env y agrega tu API key
   nano .env
   
   # Levanta los servicios
   make up
   ```

   **Opción C: Docker Compose directo**
   ```bash
   GEMINI_API_KEY="tu_api_key_aqui" docker compose up -d
   ```

3. **Verificar la configuración:**
   ```bash
   # Ver logs del microservicio LLM
   make logs service=llm_microservice
   ```

### Archivos de Configuración

- `microservice.conf`: Configuración del microservicio
- `docker-compose.yml`: Configuración de servicios Docker
- `env.example`: Ejemplo de variables de entorno
- `.env`: Variables de entorno locales (crear desde env.example)

## Testing

Ejecutar tests unitarios:

```bash
# Todos los tests
cargo test

# Tests de un componente específico
cd microservice && cargo test
```

## Troubleshooting

### Problemas Comunes

1. **Puerto ocupado:**
```bash
make stop
make down
make up
```

2. **Problemas de permisos Docker:**
```bash
sudo usermod -aG docker $USER
# Reiniciar sesión
```

3. **Limpiar completamente:**
```bash
make clean
make prune
make rebuild
```

### Logs y Debugging

```bash
# Ver logs en tiempo real
make logs service=microservice

# Acceder al contenedor para debugging
make exec service=redis_server cmd="redis-cli"
```
