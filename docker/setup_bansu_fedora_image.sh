#!/usr/bin/sh

do_wget() {
  wget --retry-connrefused --waitretry=1 --read-timeout=10 --timeout=10 -t 15 "$@" || exit 7
}

LIBEIGEN_VER=3.4.0
RDKIT_VER=2024_03_6
GEMMI_VER=0.6.7
SERVALCAT_VER=0.4.78
# ACEDRG_VER=main
ACEDRG_VER=bzr

setup_build_env() {
  export CMAKE_BUILD_PARALLEL_LEVEL=`nproc --all`
}

download_all() {
    cd /download

    # Acedrg
    # do_wget https://ccp4forge.rc-harwell.ac.uk/ccp4/acedrg/-/archive/main/acedrg-${ACEDRG_VER}.tar.gz &&\
    # tar -xf acedrg-${ACEDRG_VER}.tar.gz
    echo Checking-out acedrg with breezy \(be patient, this may take a long time\)...
    brz checkout https://fg.oisin.rc-harwell.ac.uk/anonscm/bzr/acedrg/trunk/ acedrg-${ACEDRG_VER} || exit 7


    # Libeigen
    do_wget https://gitlab.com/libeigen/eigen/-/archive/${LIBEIGEN_VER}/eigen-${LIBEIGEN_VER}.tar.gz &&\
    tar -xf eigen-${LIBEIGEN_VER}.tar.gz

    #RDKit
    do_wget https://github.com/rdkit/rdkit/archive/refs/tags/Release_${RDKIT_VER}.tar.gz &&\
    tar -xf Release_${RDKIT_VER}.tar.gz &&\
    mv rdkit-Release_${RDKIT_VER} RDKit_${RDKIT_VER}

    #GEMMI
    do_wget https://github.com/project-gemmi/gemmi/archive/refs/tags/v${GEMMI_VER}.tar.gz -O gemmi-${GEMMI_VER}.tar.gz &&\
    tar -xf gemmi-${GEMMI_VER}.tar.gz

    # Servalcat
    do_wget https://github.com/keitaroyam/servalcat/archive/refs/tags/v${SERVALCAT_VER}.tar.gz -O servalcat-${SERVALCAT_VER}.tar.gz &&\
    tar -xf servalcat-${SERVALCAT_VER}.tar.gz
}

build_eigen() {
  setup_build_env
  mkdir -p /build/eigen
  cd /build/eigen &&\
  rm -rf *
  cmake -S /download/eigen-${LIBEIGEN_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release 
  cmake --build . && cmake --install .
  cd ..
}

build_rdkit() {
  setup_build_env
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
  setup_build_env
  mkdir -p /build/gemmi
  cd /build/gemmi &&\
  rm -rf *
  cmake -S /download/gemmi-${GEMMI_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release -DUSE_PYTHON=1 -DBUILD_SHARED_LIBS=true
  cmake --build . && cmake --install .
  cd ..
}

build_acedrg() {
  setup_build_env
  mkdir -p /build/acedrg
  cd /build/acedrg &&\
  rm -rf *
  cmake -S /download/acedrg-${ACEDRG_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release 
  cmake --build . && cmake --install .
  cd ..
}

build_servalcat() {
  setup_build_env
  mkdir -p /build/servalcat
  cd /build/servalcat &&\
  rm -rf *
  cmake -S /download/servalcat-${SERVALCAT_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release 
  cmake --build . && cmake --install .
  cd ..
}


build_all() {
    build_eigen
    build_rdkit
    build_gemmi
    build_servalcat
    build_acedrg

    # Seems to be necessary for RDKit stuff to be found at runtime
    ldconfig
}

cleanup_all() {
 echo Removing /download
 rm -rf /download
 echo Removing /build
 rm -rf /build
 echo Cleanup done
}

setup_all() {
  download_all
  build_all
  cleanup_all
}

