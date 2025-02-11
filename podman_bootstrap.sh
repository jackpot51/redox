# This script setup the Redox build system with Podman
# It install the Podman dependencies, Rustup, recipes dependencies for cross-compilation
# and download the build system configuration files

#!/usr/bin/env bash

set -e

##########################################################
# This function is simply a banner to introduce the script
##########################################################
banner()
{
	echo "|------------------------------------------|"
	echo "|----- Welcome to the redox bootstrap -----|"
	echo "|-------- for building with Podman --------|"
	echo "|------------------------------------------|"
}

###################################################################################
# This function takes care of installing a dependency via package manager of choice
# for building Redox on BSDs (macOS, FreeBSD, etc.).
# @params:    $1 package manager
#             $2 package name
#             $3 binary name (optional)
###################################################################################
install_bsd_pkg()
{
    PKG_MANAGER=$1
    PKG_NAME=$2
    BIN_NAME=$3
    if [ -z "$BIN_NAME" ]; then
        BIN_NAME=$PKG_NAME
    fi

    BIN_LOCATION=$(which $BIN_NAME || true)
    if [ -z "$BIN_LOCATION" ]; then
        echo "$PKG_MANAGER install $PKG_NAME"
        $PKG_MANAGER install "$PKG_NAME"
    else
        echo "$BIN_NAME already exists at $BIN_LOCATION, no need to install $PKG_NAME..."
    fi
}

install_macports_pkg()
{
    install_bsd_pkg "sudo port" "$1" "$2"
}

install_brew_pkg()
{
    install_bsd_pkg "brew" $@
}

install_brew_cask_pkg()
{
    install_bsd_pkg "brew cask" $@
}

install_freebsd_pkg()
{
    install_bsd_pkg "sudo pkg" $@
}

###############################################################################
# This function checks which of the supported package managers
# is available on the macOS host.
# If a supported package manager is found, it delegates the installing work to
# the relevant function.
# Otherwise this function will exit this script with an error.
###############################################################################
osx()
{
    echo "Detected macOS!"

    if [ ! -z "$(which brew)" ]; then
        osx_homebrew $@
    elif [ ! -z "$(which port)" ]; then
        osx_macports $@
    else
        echo "Please install either Homebrew or MacPorts, if you wish to use this script"
        echo "Re-run this script once you installed one of those package managers"
        echo "Will not install, now exiting..."
        exit 1
    fi
}

###############################################################################
# This function takes care of installing all dependencies using MacPorts
# for building Redox on macOS
# @params:    $1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
osx_macports()
{
    echo "MacPorts detected! Now updating..."
    sudo port -v selfupdate

    echo "Installing missing packages..."

    install_macports_pkg "git"
	install_macports_pkg "gmake"
	install_macports_pkg "curl"
	install_macports_pkg "osxfuse"
	install_macports_pkg "podman"

	if [ "$1" == "qemu" ]; then
        install_macports_pkg "qemu" "qemu-system-x86_64"
	elif [ "$1" == "virtualbox" ]; then
        install_macports_pkg "virtualbox"
    else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

}

###############################################################################
# This function takes care of installing all dependencies using Homebrew
# for building Redox on macOS
# @params:    $1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
osx_homebrew()
{
    echo "Homebrew detected! Now updating..."
    brew update

    echo "Installing missing packages..."

    install_brew_pkg "git"
	install_brew_pkg "make"
	install_brew_pkg "curl"
	install_brew_pkg "osxfuse"
	install_brew_pkg "fuse-overlayfs"
	install_brew_pkg "slirp4netns"
	install_brew_pkg "podman"

	if [ "$1" == "qemu" ]; then
        install_brew_pkg "qemu" "qemu-system-x86_64"
    elif [ "$1" == "virtualbox" ]; then
        install_brew_pkg "virtualbox"
    else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

}

###############################################################################
# This function takes care of installing all dependencies using pkg
# for building Redox on FreeBSD
# @params:    $1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
freebsd()
{
    set -x
    echo "FreeBSD detected!"
    echo "Installing missing packages..."

    install_freebsd_pkg "git"
    install_freebsd_pkg "gmake"
	install_freebsd_pkg "curl"
	install_freebsd_pkg "fusefs-libs3"
    install_freebsd_pkg "podman"


	if [ "$1" == "qemu" ]; then
        install_freebsd_pkg "qemu" "qemu-system-x86_64"
    elif [ "$1" == "virtualbox" ]; then
        install_freebsd_pkg "virtualbox"
    else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

    set +x
}

###############################################################################
# This function takes care of installing all dependencies for building Redox on
# Arch Linux
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
archLinux()
{

	echo "Detected Arch Linux"
	packages="git make curl fuse3 fuse-overlayfs slirp4netns podman"
	if [ "$1" == "qemu" ]; then
		packages="$packages qemu"
	elif [ "$1" == "virtualbox" ]; then
		packages="$packages virtualbox"
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi
	# Scripts should not cause a system update in order to just install a couple
	#   of packages. If pacman -S --needed is going to fail, let it fail and the
	#   user will figure out the issues (without updating if required) and rerun
	#   the script.
	#echo "Updating system..."
	#sudo pacman -Syu

	echo "Installing packages $packages..."
	sudo pacman -S --needed $packages
	
}

###############################################################################
# This function takes care of installing all dependencies for building Redox on
# Debian-based Linux
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
# 		$2 the package manager to use
###############################################################################
ubuntu()
{
	echo "Detected Ubuntu/Debian"
	echo "Updating system..."
	sudo "$2" update
	echo "Installing required packages..."
	sudo "$2" install \
		podman curl git make pkg-config fuse3 libfuse3-dev fuse-overlayfs slirp4netns
	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			echo "Installing QEMU..."
			sudo "$2" install qemu-system-x86 qemu-kvm
			sudo "$2" install qemu-efi-arm qemu-system-arm
		else
			echo "QEMU already installed!"
		fi
	elif [ "$1" == "virtualbox" ]; then
		if [ -z "$(which virtualbox)" ]; then

			if grep '^ID=debian$' /etc/os-release > /dev/null; then
				echo "Virtualbox is not in the official debian packages"
				echo "To install virtualbox on debian, see https://wiki.debian.org/VirtualBox"
				echo "Please install VirtualBox and re-run this script,"
				echo "or run with -e qemu"
				exit 1
			else
				echo "Installing VirtualBox..."
				sudo "$2" install virtualbox
			fi
		else
			echo "VirtualBox already installed!"
		fi
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

}

###############################################################################
# This function takes care of installing all dependencies for building Redox on
# Fedora Linux
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
fedora()
{
	echo "Detected Fedora"
	if [ -z "$(which git)" ]; then
		echo "Installing git..."
		sudo dnf install git-all
	fi

	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			echo "Installing QEMU..."
			sudo dnf install qemu-system-x86 qemu-kvm
		else
			echo "QEMU already installed!"
		fi
	elif [ "$1" == "virtualbox" ]; then
		if [ -z "$(which virtualbox)" ]; then
			echo "Please install VirtualBox and re-run this script,"
			echo "or run with -e qemu"
			exit 1
		else
			echo "VirtualBox already installed!"
		fi
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi
		
	# Use rpm -q <package> to check if it's already installed
	PKGS=$(for pkg in podman curl make fuse3 fuse-overlayfs slirp4netns; do rpm -q $pkg > /dev/null || echo $pkg; done)
	# If the list of packages is not empty, install missing
	COUNT=$(echo $PKGS | wc -w)
	if [ $COUNT -ne 0 ]; then
					echo "Installing necessary build tools..."
					sudo dnf install $PKGS
	fi
}

###############################################################################
# This function takes care of installing all dependencies for building Redox on
# *SUSE Linux
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
###############################################################################
suse()
{
	echo "Detected SUSE Linux"

	packages=(
		"git"
		"curl"
		"make"
		"fuse"
		"fuse-overlayfs"
		"slirp4netns"
		"podman"
	)

	if [ -z "$(which git)" ]; then
		echo "Will install git ..."
		packages+=(git)
	fi

	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			echo "Will install QEMU..."
			packages+=(qemu-x86 qemu-kvm)
		else
			echo "QEMU already installed!"
		fi
	elif [ "$1" == "virtualbox" ]; then
		if [ -z "$(which virtualbox)" ]; then
			echo "Please install VirtualBox and re-run this script,"
			echo "or run with -e qemu"
			exit 1
		else
			echo "VirtualBox already installed!"
		fi
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi
	
	echo "Installing necessary build tools..."

	# We could install all the packages in a single zypper command with:
	#
	#        zypper install package1 package2 package3
	# 
	# But there is an issue with this: zypper returns a success code if at
	# least one of the packages was correctly installed, but we need it to fail
	# if any of the packages is missing.
	#
	# To confirm that the packages are available, we try to install them one by
	# one with --dry-run.
	# We still install all the packages in a single zypper command so that the
	# user has to confirm only once.
	for p in ${packages[@]}; do
		if rpm -q "${p}" > /dev/null ; then
		   echo "${p} is already installed"
		else
		   # Zypper shows a confirmation prompt and the "y" answer even with 
		   # --non-interactive and --no-confirm:
		   #
		   #   1 new package to install.
           #   Overall download size: 281.7 KiB. Already cached: 0 B. After the operation, additional 394.6 KiB will be used.
           #   Continue? [y/n/v/...? shows all options] (y): y
		   #
		   # That could make the user think that the package was installed, 
		   # when it was only a dry run.
		   # To avoid the confusion, we hide the output unless there was an
		   # error.
		   if out="$(zypper --non-interactive install --no-confirm --dry-run --force-resolution ${p}  2>&1)"  ; then
		      echo "${p} can be installed"
		   else
		   	  echo "no"
			  echo ""
		      echo "Zypper output:"
			  echo ""
			  echo "${out}"
			  echo ""
		      echo "Could not find how to install '${p}', try running:"
			  echo ""
			  echo "     zypper install ${p}"
			  echo ""
			  exit 1
		   fi
		fi
	done

	zypper install ${packages[@]}

}

##############################################################################
# This function takes care of installing all dependencies for building Redox on
# Gentoo Linux
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
##############################################################################
gentoo()
{
	echo "Detected Gentoo Linux"
	if [ -z "$(which git)" ]; then
		echo "Installing git..."
		sudo emerge dev-vcs/git
	fi
	if [ -z "$(which fusermount 2>/dev/null)" ] && [ -z "$(which fusermount3 2>/dev/null)" ]; then
		echo "Installing fuse..."
		sudo emerge sys-fs/fuse
	fi

	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			echo "Please install QEMU and re-run this script"
			echo "Step1. Add QEMU_SOFTMMU_TARGETS=\"x86_64\" to /etc/portage/make.conf"
			echo "Step2. Execute \"sudo emerge app-emulation/qemu\""
            exit 1
		else
			echo "QEMU already installed!"
		fi
	elif [ "$1" == "virtualbox" ]; then
		if [ -z "$(which virtualbox)" ]; then
			echo "Please install VirtualBox and re-run this script,"
			echo "or run with -e qemu"
			exit 1
		else
			echo "VirtualBox already installed!"
		fi
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

	if [ -z "$(which cmake)" ]; then
		echo "Installing cmake..."
		sudo emerge dev-util/cmake
	fi
	if [ -z "$(which podman)" ]; then
		echo "Please install Podman, https://wiki.gentoo.org/wiki/Podman"
		exit 1
	fi
}

##############################################################################
# This function takes care of installing all dependencies for building Redox on
# Solus
# @params:	$1 the emulator to install, "virtualbox" or "qemu"
##############################################################################
solus()
{
	echo "Detected Solus"

	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			sudo eopkg it qemu
		else
			echo "QEMU already installed!"
		fi
	elif [ "$1" == "virtualbox" ]; then
		if [ -z "$(which virtualbox)" ]; then
			echo "Please install VirtualBox and re-run this script,"
			echo "or run with -e qemu"
			exit 1
		else
			echo "VirtualBox already installed!"
		fi
	else
	   echo "Unknown emulator: $1"
	   exit 1
	fi

	echo "Installing necessary build tools..."
	#if guards are not necessary with eopkg since it does nothing if latest version is already installed
	sudo eopkg it fuse-devel git make fuse2-devel rsync
	if [ -z "$(which podman)" ]; then
		echo "Please install Podman"
		exit 1
	fi
}

##############################################################################
# This function takes care of installing all dependencies for building redox on
# VoidLinux
# @params:	$1 the emulator to install, virtualbox or qemu
##############################################################################
voidlinux()
{

	echo "Detected VoidLinux"

	if [ "$1" == "qemu" ]; then
		if [ -z "$(which qemu-system-x86_64)" ]; then
			sudo xbps-install -S qemu
		else
			echo "QEMU already installed!"
		fi
	else
		if [ -z "$(which virtualbox)" ]; then
			echo "Please install Virtualbox and re-run this script,"
			echo "or run with -e qemu"
			exit
		else
			echo "Virtualbox already installed!"
		fi
	fi

	echo "Installing necessary build tools..."
	sudo xbps-install -S gcc make nasm wget pkg-config fuse-devel rsync automake gettext-devel libtool po4a patch bison flex gperf libpng-devel perl-HTML-Parser
}

######################################################################
# This function outlines the different options available for bootstrap
######################################################################
usage()
{
	echo "------------------------"
	echo "|Redox bootstrap script|"
	echo "------------------------"
	echo "Usage: ./podman_bootstrap.sh"
	echo "OPTIONS:"
	echo
	echo "   -h,--help      Show this prompt"
	echo "   -u [branch]    Update git repo and update rust"
	echo "                  If blank defaults to master"
	echo "   -s             Check the status of the current travis build"
	echo "   -e [emulator]  Install specific emulator, virtualbox or qemu"
	echo "   -p [package    Choose an Ubuntu package manager, apt-fast or"
	echo "       manager]   aptitude"
	echo "   -d             Only install the dependencies, skip boot step"
	echo "EXAMPLES:"
	echo
	echo "./podman_bootstrap.sh -e qemu"
	exit
}


#############################################################
# Looks for and installs a cargo-managed binary or subcommand
#############################################################
cargoInstall() {
	if [[ "`cargo install --list`" != *"$1 v$2"* ]]; then
		cargo install --force --version "$2" "$1"
	else
		echo "You have $1 version $2 installed already!"
	fi
}


####################################################################################
# This function takes care of everything associated to rust, and the version manager
# That controls it, it can install rustup and uninstall multirust as well as making
# sure that the correct version of rustc is selected by rustup
# @params:	$1 install non-interactively, boolean
####################################################################################
rustInstall() {
	noninteractive=$1
	# Check to see if multirust is installed, we don't want it messing with rustup
	# In the future we can probably remove this but I believe it's good to have for now
	if [ -e /usr/local/lib/rustlib/uninstall.sh ] ; then
		echo "It appears that multirust is installed on your system."
		echo "This tool has been deprecated by the maintainer, and will cause issues."
		echo "This script can remove multirust from your system if you wish."
		printf "Uninstall multirust (y/N):"
		read multirust
		if echo "$multirust" | grep -iq "^y" ;then
			sudo /usr/local/lib/rustlib/uninstall.sh
		else
			echo "Please manually uninstall multirust and any other versions of rust, then re-run bootstrap."
			exit 1
		fi
	fi
	# If rustup is not installed we should offer to install it for them
	if [ -z "$(which rustup)" ]; then
        rustup_options="--default-toolchain stable"
		echo "You do not have rustup installed."
		if [ "$noninteractive" = true ]; then
		   rustup="y"
		   rustup_options+=" -y"
		else
			echo "We HIGHLY recommend using rustup."
			echo "Would you like to install it now?"
			echo "*WARNING* this involves a 'curl | sh' style command"
			printf "(y/N): "
			read rustup
		fi
		if echo "$rustup" | grep -iq "^y" ;then
			#install rustup
			curl https://sh.rustup.rs -sSf | sh -s -- $rustup_options
			# You have to add the rustup variables to the $PATH
			echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> ~/.bashrc
			# source the variables so that we can execute rustup commands in the current shell
			source ~/.cargo/env
		else
			echo "Rustup will not be installed!"
		fi
	fi
	#
	if [ -z "$(which rustc)" ]; then
		echo "Rust is not installed"
		echo "Please either run the script again, accepting rustup install"
		echo "or install rustc stable manually (not recommended) via:"
		echo "\#curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=stable"
		exit 1
	else
		echo "Your Rust install looks good!"
	fi
}

####################################################################
# This function gets the current build status from travis and prints
# a message to the user
####################################################################
statusCheck() {
	for i in $(echo "$(curl -sf https://api.travis-ci.org/repositories/redox-os/redox.json)" | tr "," "\n")
	do
		if echo "$i" | grep -iq "last_build_status" ;then
			if echo "$i" | grep -iq "0" ;then
				echo
				echo "********************************************"
				echo "Travis reports that the last build succeeded!"
				echo "Looks like you are good to go!"
				echo "********************************************"
			elif echo "$i" | grep -iq "null" ;then
				echo
				echo "******************************************************************"
				echo "The Travis build did not finish, this is an error with its config."
				echo "I cannot reliably determine whether the build is succeeding or not."
				echo "Consider checking for and maybe opening an issue on gitlab"
				echo "******************************************************************"
			else
				echo
				echo "**************************************************"
				echo "Travis reports that the last build *FAILED* :("
				echo "Might want to check out the issues before building"
				echo "**************************************************"
			fi
		fi
	done
}

###########################################################################
# This function is the main logic for the bootstrap; it clones the git repo
# then it installs the dependent packages
###########################################################################
boot()
{
	echo "Cloning gitlab repo..."
	git clone https://gitlab.redox-os.org/redox-os/redox.git --origin upstream --recursive
	echo "Creating .config with PODMAN_BUILD=1"
	echo 'PODMAN_BUILD?=1' > redox/.config
	echo "Cleaning up..."
	rm podman_bootstrap.sh
	echo
	echo "---------------------------------------"
	echo "Well it looks like you are ready to go!"
	echo "---------------------------------------"
	statusCheck
	echo "The file redox/.config was created with PODMAN_BUILD=1."
	echo
	echo "** Be sure to update your path to include Rust - run the following command: **"
	echo 'source $HOME/.cargo/env'
	echo
	echo "Run the following commands to build redox using Podman:"
	echo
	echo "cd redox"
	MAKE="make"
	if [[ "$(uname)" == "FreeBSD" ]]; then
    	MAKE="gmake"
    	echo "kldload fuse.ko # This loads the kernel module for FUSE"
    fi
	echo "$MAKE all"
	echo "$MAKE virtualbox or qemu"
	echo
	echo "      Good luck!"

	exit
}

if [ "$1" == "-h" ] || [ "$1" == "--help" ]; then
	usage
elif [ "$1" == "-u" ]; then
	git pull upstream master
	git submodule update --recursive --init
	rustup update nightly
	exit
elif [ "$1" == "-s" ]; then
	statusCheck
	exit
fi

emulator="qemu"
defpackman="apt-get"
dependenciesonly=false
update=false
while getopts ":e:p:udhs" opt
do
	case "$opt" in
		e) emulator="$OPTARG";;
		p) defpackman="$OPTARG";;
		d) dependenciesonly=true;;
		u) update=true;;
		h) usage;;
		s) statusCheck && exit;;
		\?) echo "I don't know what to do with that option, try -h for help"; exit 1;;
	esac
done

banner

rustInstall "$noninteractive"

if [ "$update" == "true" ]; then
	git pull upstream master
	git submodule update --recursive --init
	exit
fi

if [ "Darwin" == "$(uname -s)" ]; then
	osx "$emulator"
else
	# Here we will use package managers to determine which operating system the user is using.

	# SUSE and derivatives
	if hash 2>/dev/null zypper; then
		suse "$emulator"
	# Debian or any derivative of it
	elif hash 2>/dev/null apt-get; then
		ubuntu "$emulator" "$defpackman"
	# Fedora
	elif hash 2>/dev/null dnf; then
		fedora "$emulator"
	# Gentoo
	elif hash 2>/dev/null emerge; then
		gentoo "$emulator"
	# Solus
	elif hash 2>/dev/null eopkg; then
		solus "$emulator"
	# Arch Linux
	elif hash 2>/dev/null pacman; then
		archLinux "$emulator"
	# FreeBSD
	elif hash 2>/dev/null pkg; then
		freebsd "$emulator"
	elif hash 2>/dev/null xbps-install; then
	  voidlinux "$emulator"
	# Unsupported platform
	else
    	printf "\e[31;1mFatal error: \e[0;31mUnsupported platform, please open an issue\e[0m\n"
	fi
fi

if [ "$dependenciesonly" = false ]; then
	boot
fi

echo "Redox bootstrap complete!"