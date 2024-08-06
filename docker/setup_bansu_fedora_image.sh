#!/usr/bin/sh


initial_setup() {
    mkdir -p /download
    mkdir -p /build
    dnf install -y git gcc gcc-c++ wget gunzip python boost cmake pybind11-devel
}

do_wget() {
  wget --retry-connrefused --waitretry=1 --read-timeout=10 --timeout=10 -t 15 "$@" || exit 7
}

RDKIT_VER=2024_03_2
GEMMI_VER=0.6.6
SERVALCAT_VER=0.4.77


download_all() {
    cd /download

    # Acedrg
    do_wget https://ccp4forge.rc-harwell.ac.uk/ccp4/acedrg/-/archive/main/acedrg-main.tar.gz &&\
    tar -xf acedrg-main.tar.gz

    #RDKit
    do_wget https://github.com/rdkit/rdkit/archive/refs/tags/Release_${RDKIT_VER}.tar.gz &&\
    tar -xf Release_${RDKIT_VER}.tar.gz &&\
    mv rdkit-Release_${RDKIT_VER} RDKit_${RDKIT_VER}

    #GEMMI
    do_wget https://github.com/project-gemmi/gemmi/archive/refs/tags/v${GEMMI_VER}.tar.gz -O gemmi-${GEMMI_VER}.tar.gz &&\
    tar -xf gemmi-${GEMMI_VER}.tar.gz

}

build_rdkit() {
  mkdir -p /build/rdkit
  cd /build/rdkit &&\
  rm -rf *
  cmake -S /download/RDKit_${RDKIT_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release \
  -DRDK_BUILD_CAIRO_SUPPORT=OFF \
  -DRDK_BUILD_INCHI_SUPPORT=OFF \
  -DRDK_INSTALL_COMIC_FONTS=OFF \
  -DRDK_INSTALL_INTREE=OFF 

  cmake --build . && cmake --install .
  cd ..
}

build_gemmi() {
  mkdir -p /build/gemmi
  cd /build/gemmi &&\
  rm -rf *
  cmake -S /download/gemmi-${GEMMI_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release -DUSE_PYTHON=1 -DBUILD_SHARED_LIBS=true
  cmake --build . && cmake --install .
  cd ..
}

build_acedrg() {

}

build_servalcat() {

}


build_all() {
    build_rdkit
    build_gemmi
    build_servalcat
    build_acedrg
}

cleanup_all() {

}

initial_setup
download_all
build_all
clenup_all

