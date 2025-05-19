# Taller de Programación - A Todo Rust

## Integrantes

- Camila General  
- Ramiro Mantero  
- Valentina Moreno  
- Franco Secchi  

## Requisitos previos

Para compilar y ejecutar este proyecto, es necesario tener instalado:

- [Rust](https://www.rust-lang.org/tools/install)
- Cargo (incluido con Rust)
- Las siguientes dependencias del sistema para compilar con `Relm4` y `GTK4`:

```bash
sudo apt-get install -y \
    libglib2.0-dev \
    pkg-config \
    libpango1.0-dev \
    libgdk-pixbuf2.0-dev \
    libgtk-4-dev
```

### Como correr

Para generar un nodo y un cliente, por un lado hay que ejecutar


```bash
cargo run --bin node 4000
```
Y en otra terminal 
```bash
cargo run --bin client
```


## Como testear
