cmake_minimum_required(VERSION 3.22)

project(physbox5_box3d_planar_observer LANGUAGES C CXX)

set(PHYSBOX5_BOX3D_SOURCE "" CACHE PATH "Path to the clean pinned Box3D source")
if(NOT PHYSBOX5_BOX3D_SOURCE)
	message(FATAL_ERROR "PHYSBOX5_BOX3D_SOURCE must name the clean pinned Box3D checkout")
endif()

set(PHYSBOX5_BOX3D_PIN "8441b4a06d6d09dcfb0b0f704df4d847d1437b92")
execute_process(
	COMMAND git -C "${PHYSBOX5_BOX3D_SOURCE}" rev-parse HEAD
	OUTPUT_VARIABLE PHYSBOX5_BOX3D_HEAD
	OUTPUT_STRIP_TRAILING_WHITESPACE
	COMMAND_ERROR_IS_FATAL ANY
)
if(NOT PHYSBOX5_BOX3D_HEAD STREQUAL PHYSBOX5_BOX3D_PIN)
	message(FATAL_ERROR "Box3D source is ${PHYSBOX5_BOX3D_HEAD}; expected ${PHYSBOX5_BOX3D_PIN}")
endif()

execute_process(
	COMMAND git -C "${PHYSBOX5_BOX3D_SOURCE}" status --porcelain=v1 --untracked-files=all
	OUTPUT_VARIABLE PHYSBOX5_BOX3D_STATUS
	OUTPUT_STRIP_TRAILING_WHITESPACE
	COMMAND_ERROR_IS_FATAL ANY
)
if(PHYSBOX5_BOX3D_STATUS)
	message(FATAL_ERROR "Box3D source is dirty:\n${PHYSBOX5_BOX3D_STATUS}")
endif()

set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BOX3D_VALIDATE ON CACHE BOOL "" FORCE)
set(BOX3D_DISABLE_SIMD OFF CACHE BOOL "" FORCE)
set(BOX3D_DOUBLE_PRECISION OFF CACHE BOOL "" FORCE)
set(BOX3D_PROFILE OFF CACHE BOOL "" FORCE)
set(BOX3D_COMPILE_WARNING_AS_ERROR ON CACHE BOOL "" FORCE)

add_subdirectory("${PHYSBOX5_BOX3D_SOURCE}" box3d EXCLUDE_FROM_ALL)

add_executable(box3d_planar_observer observer.c)
target_link_libraries(box3d_planar_observer PRIVATE box3d::box3d)
set_target_properties(box3d_planar_observer PROPERTIES
	C_STANDARD 17
	C_STANDARD_REQUIRED YES
	C_EXTENSIONS NO
	RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
)

if(MSVC)
	target_compile_options(box3d_planar_observer PRIVATE /W4 /WX)
else()
	target_compile_options(box3d_planar_observer PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()
