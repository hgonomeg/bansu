#!/usr/bin/sh

do_wget() {
  wget --retry-connrefused --waitretry=1 --read-timeout=10 --timeout=10 -t 15 "$@" || exit 7
}

AARDVARK_VER=0.2.0
# Currently, gemmi build fails if we use anything newer than eigen 3
LIBEIGEN_VER=3.4.0
# Acrdrg currently does not support 2026_03_3
RDKIT_VER=2024_03_2
GEMMI_VER=0.7.5
SERVALCAT_VER=0.4.142
# ACEDRG_VER=main
ACEDRG_VER=391

COOT_FORK=pemsley
COOT_COMMIT=da1b5ee8e33eb9c7d4f449fe9b05465d15b860ab
FFTW2_VER=2.1.5
MMDB2_VER=2.0.22
LIBCCP4_VER=8.0.0
LIBSSM_VER=1.4
CLIPPER_VER=2.1.20201109
CLIPPER_DIR=clipper-2.1

setup_build_env() {
  export CMAKE_BUILD_PARALLEL_LEVEL=`nproc --all`
}

download_all() {
    cd /download

    # Acedrg
    # do_wget https://ccp4forge.rc-harwell.ac.uk/ccp4/acedrg/-/archive/main/acedrg-${ACEDRG_VER}.tar.gz &&\
    # tar -xf acedrg-${ACEDRG_VER}.tar.gz
    echo Checking-out acedrg with breezy \(be patient, this may take a long time\)...
    brz checkout --light -r ${ACEDRG_VER} https://fg.oisin.rc-harwell.ac.uk/anonscm/bzr/acedrg/trunk/ acedrg-${ACEDRG_VER} || exit 7


    # Libeigen
    do_wget https://gitlab.com/libeigen/eigen/-/archive/${LIBEIGEN_VER}/eigen-${LIBEIGEN_VER}.tar.gz &&\
    tar -xf eigen-${LIBEIGEN_VER}.tar.gz || exit 7

    # RDKit
    do_wget https://github.com/rdkit/rdkit/archive/refs/tags/Release_${RDKIT_VER}.tar.gz &&\
    tar -xf Release_${RDKIT_VER}.tar.gz &&\
    mv rdkit-Release_${RDKIT_VER} RDKit_${RDKIT_VER} || exit 7

    # GEMMI
    do_wget https://github.com/project-gemmi/gemmi/archive/refs/tags/v${GEMMI_VER}.tar.gz -O gemmi-${GEMMI_VER}.tar.gz &&\
    tar -xf gemmi-${GEMMI_VER}.tar.gz || exit 7

    # Servalcat
    do_wget https://github.com/keitaroyam/servalcat/archive/refs/tags/v${SERVALCAT_VER}.tar.gz -O servalcat-${SERVALCAT_VER}.tar.gz &&\
    tar -xf servalcat-${SERVALCAT_VER}.tar.gz || exit 7

    # Aardvark
    do_wget https://github.com/hgonomeg/aardvark/archive/refs/tags/v${AARDVARK_VER}.tar.gz -O aardvark-${AARDVARK_VER}.tar.gz &&\
    tar -xf aardvark-${AARDVARK_VER}.tar.gz || exit 7

    # FFTW2
    do_wget https://www.fftw.org/fftw-${FFTW2_VER}.tar.gz &&\
    tar -xf fftw-${FFTW2_VER}.tar.gz || exit 7

    # mmdb2
    do_wget https://www2.mrc-lmb.cam.ac.uk/personal/pemsley/coot/dependencies/mmdb2-${MMDB2_VER}.tar.gz &&\
    tar -xf mmdb2-${MMDB2_VER}.tar.gz || exit 7

    # libccp4
    do_wget https://www2.mrc-lmb.cam.ac.uk/personal/pemsley/coot/dependencies/libccp4-${LIBCCP4_VER}.tar.gz &&\
    tar -xf libccp4-${LIBCCP4_VER}.tar.gz || exit 7

    # libssm
    do_wget https://www2.mrc-lmb.cam.ac.uk/personal/pemsley/coot/dependencies/ssm-${LIBSSM_VER}.tar.gz -O ssm-${LIBSSM_VER}.tar.gz &&\
    tar -xf ssm-${LIBSSM_VER}.tar.gz &&\
    do_wget "https://aur.archlinux.org/cgit/aur.git/plain/ssm.pc.in?h=libssm" -O ssm-${LIBSSM_VER}/ssm.pc.in || exit 7

    # libclipper
    do_wget https://deb.debian.org/debian/pool/main/c/clipper/clipper_${CLIPPER_VER}.orig.tar.gz -O clipper-${CLIPPER_VER}.tar.gz &&\
    tar -xf clipper-${CLIPPER_VER}.tar.gz || exit 7

    # Coot / chapi
    git clone https://github.com/${COOT_FORK}/coot.git coot &&\
    git -C coot checkout ${COOT_COMMIT} || exit 7
}

build_aardvark() {
  setup_build_env
  mkdir -p /build/aardvark
  cd /build/aardvark &&\
  rm -rf *
  g++ /download/aardvark-${AARDVARK_VER}/Pauls_COD_stuff/cod_db.cpp -o cod_db -std=c++17 -O3 -lsqlite3 &&\
  install -m 755 cod_db /usr/bin/cod_db &&\
  install -m 755 /download/aardvark-${AARDVARK_VER}/Pauls_COD_stuff/cod_bridge.py /usr/bin/aardvark.py || exit 8
  cd /build
}

build_eigen() {
  setup_build_env
  mkdir -p /build/eigen
  cd /build/eigen &&\
  rm -rf *
  cmake -S /download/eigen-${LIBEIGEN_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}

build_rdkit() {
  setup_build_env
  sed -i 's/_Py_IsFinalizing/Py_IsFinalizing/' /download/RDKit_${RDKIT_VER}/Code/RDBoost/Wrap/RDBase.cpp
  mkdir -p /build/rdkit
  cd /build/rdkit &&\
  rm -rf *
  cmake -S /download/RDKit_${RDKIT_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release \
  -DCMAKE_C_FLAGS="-std=gnu17" \
  -DRDK_BUILD_CAIRO_SUPPORT=OFF \
  -DRDK_BUILD_INCHI_SUPPORT=OFF \
  -DRDK_BUILD_FREETYPE_SUPPORT=OFF \
  -DRDK_INSTALL_COMIC_FONTS=OFF \
  -DRDK_INSTALL_INTREE=OFF  &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}

build_gemmi() {
  setup_build_env
  mkdir -p /build/gemmi
  cd /build/gemmi &&\
  rm -rf *
  cmake -S /download/gemmi-${GEMMI_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release -DUSE_PYTHON=1 -DBUILD_SHARED_LIBS=true &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}

build_acedrg() {
  setup_build_env
  mkdir -p /build/acedrg
  cd /build/acedrg &&\
  rm -rf *
  cmake -S /download/acedrg-${ACEDRG_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}

build_servalcat() {
  setup_build_env
  mkdir -p /build/servalcat
  cd /build/servalcat &&\
  rm -rf *
  cmake -S /download/servalcat-${SERVALCAT_VER} \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}


build_fftw2() {
  setup_build_env
  cd /download/fftw-${FFTW2_VER} &&\
  ./configure --prefix=/usr --enable-shared --disable-static --with-gcc --with-gnu-ld &&\
  make -j`nproc --all` && make install || exit 8
  cd /build
}

build_mmdb2() {
  setup_build_env
  cd /download/mmdb2-${MMDB2_VER} &&\
  FFLAGS="-std=f2008 -fallow-argument-mismatch" \
  ./configure --prefix=/usr --enable-shared &&\
  make -j`nproc --all` && make install || exit 8
  cd /build
}

build_libccp4() {
  setup_build_env
  cd /download/libccp4-${LIBCCP4_VER} &&\
  FFLAGS="-std=f2008 -fallow-argument-mismatch" \
  CFLAGS="-Wno-incompatible-pointer-types -std=gnu17" \
  ./configure --prefix=/usr --enable-shared --disable-static --datadir=/usr/share/ccp4 &&\
  make -j`nproc --all` && make install || exit 8
  cd /build
}

build_libssm() {
  setup_build_env
  cd /download/ssm-${LIBSSM_VER} &&\
  aclocal && libtoolize --automake --copy && autoconf && automake --copy --add-missing --gnu &&\
  ./configure --prefix=/usr --enable-shared --disable-static --enable-ccp4 &&\
  make -j`nproc --all` && make install || exit 8
  cd /build
}

build_libclipper() {
  setup_build_env
  sed -i 's/from >> &word\[0\]/from >> word/' /download/${CLIPPER_DIR}/clipper/cif/cif_data_io.cpp
  cd /download/${CLIPPER_DIR} &&\
  CXXFLAGS="-O2 -fno-strict-aliasing -Wno-narrowing" \
  CFLAGS="-O2 -fno-strict-aliasing -Wno-narrowing" \
  FFLAGS="-std=f2008 -fallow-argument-mismatch" \
  ./configure --prefix=/usr --enable-shared --disable-static \
    --enable-contrib --enable-ccp4 --enable-cif --enable-mmdb --enable-minimol \
    --enable-cns --enable-phs --enable-fortran &&\
  make -j`nproc --all` && make install || exit 8
  cd /build
}

build_chapi() {
  setup_build_env
  ldconfig
  mkdir -p /build/chapi
  cd /build/chapi &&\
  rm -rf *
  cmake -S /download/coot \
  -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_BUILD_TYPE=release \
  -Dnanobind_DIR=`python3 -m nanobind --cmake_dir` &&\
  cmake --build . && cmake --install . || exit 8
  cd ..
}

build_all() {
    build_eigen &&\
    build_rdkit &&\
    build_gemmi &&\
    build_servalcat &&\
    build_acedrg &&\
    build_aardvark &&\
    build_fftw2 &&\
    build_mmdb2 &&\
    build_libccp4 &&\
    build_libssm &&\
    build_libclipper &&\
    build_chapi || exit 8

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
  build_all || exit 8
  cleanup_all
}

