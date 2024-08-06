#!/usr/bin/sh


initial_setup() {
    mkdir -p /download
    mkdir -p /build
    dnf update -y
    dnf install -y git gcc gcc-c++ wget gunzip python python-devel \
      boost boost-devel cmake pybind11-devel python-pybind11 meson \
      gmake pip bison flex \
      helix vim
    pip install setuptools numpy
}

do_wget() {
  wget --retry-connrefused --waitretry=1 --read-timeout=10 --timeout=10 -t 15 "$@" || exit 7
}

RDKIT_VER=2024_03_5
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

    # Servalcat
    do_wget https://github.com/keitaroyam/servalcat/archive/refs/tags/v${SERVALCAT_VER}.tar.gz &&\
    tar -xf servalcat-${SERVALCAT_VER}.tar.gz

}

build_rdkit() {
  mkdir -p /build/rdkit
  cd /build/rdkit &&\
  rm -rf *
  cmake -S /download/RDKit_${RDKIT_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release \
  -DRDK_BUILD_CAIRO_SUPPORT=OFF \
  -DRDK_BUILD_INCHI_SUPPORT=OFF \
  -DRDK_BUILD_FREETYPE_SUPPORT=OFF \
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

# build_acedrg() {
#   # something
# }

# build_servalcat() {
#  # something
# }


build_all() {
    build_rdkit
    build_gemmi
    build_servalcat
    build_acedrg
    echo duppa
}

cleanup_all() {
 echo Removing /download
 rm -rf /download
 echo Removing /build
 rm -rf /build
 echo Cleanup done
}

initial_setup
download_all
build_all
# cleanup_all

