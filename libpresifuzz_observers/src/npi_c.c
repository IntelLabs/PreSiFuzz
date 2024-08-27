// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

/* --------------------------------------------------------------------------------
 * Description:
 * NPI Coverage Model using C/C++
 * This example demonstrates
 * 1. Open a coverage databse.
 * 2. Merge test.
 * 3. Traverse instance from top
 * 4. Copy to map selected metric
 * -------------------------------------------------------------------------------- */

#include "stdio.h"
#ifndef DUMMY_LIB
#include "npi.h"
#include "npi_cov.h"
#include "npi_L1.h"
#include "npi_fsdb.h"
#else
typedef void* npiCovHandle;
#endif

#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/stat.h>        /* For mode constants */
#include <fcntl.h>           /* For O_* constants */
#include <unistd.h>
#include <sys/types.h>
#include <string>
#include <ctime>

#include <vector>
#include <utility>  // for std::pair
#include <cstdlib>  // for rand(), srand()

#include "stdint.h"

#ifdef __cplusplus
extern "C" {
#endif

  typedef struct {
    uint32_t* map;
    unsigned write_byte_index;
    unsigned write_bit_index;
    unsigned type;
    unsigned size;
    uint32_t coverable;
    uint32_t covered;
    char*    filter;
  }CoverageMap;
  
  void instance_map_size( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map);
  void compute_size( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map);
  size_t compute_map_size(npiCovHandle db, unsigned coverage_type, char* filter);

  void dump_instance_coverage( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map);
  npiCovHandle vdb_cov_init(const char* vdb_file_path);
  void vdb_cov_end(npiCovHandle db);
  void compute_score( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map);
  void update_cov_map(npiCovHandle db, uint32_t* map, unsigned map_size, unsigned coverage_type, char* filter);
  void vdb_init();

  std::vector<std::pair<uint64_t, const char*>>*  fsdb_sig_value_between(npiFsdbFileHandle file_hdl, const char* sig_name, npiFsdbTime begin_time, npiFsdbTime end_time, npiFsdbValType val_type);
  
  npiFsdbFileHandle fsdb_open(const char* fsdb_filename);

  void fsdb_close(npiFsdbFileHandle file_hdl);

  void fsdb_end();

  void fsdb_init();

  int fsdb_lib_sig_value_between( npiFsdbFileHandle file, 
    const char* sigName, 
    const npiFsdbTime &beginTime, 
    const npiFsdbTime &endTime, 
    fsdbTimeValPairVec_t &vc /*O*/, 
    const npiFsdbValType &format );

  npiFsdbFileHandle fsdb_open(const NPI_BYTE8* fsdb_filename) {
    return npi_fsdb_open(fsdb_filename);
  }
  
  void fsdb_close(npiFsdbFileHandle file_hdl) {
    npi_fsdb_close(file_hdl);
  }

int fsdb_lib_val_to_str( const npiFsdbValue &value, string &str /*O*/ ) {
  char buf[32];// 32: the digit of 64 bits integer is less or equal to 20 
  
  switch( value.format ) {
    case npiFsdbBinStrVal:
    case npiFsdbOctStrVal:
    case npiFsdbDecStrVal:
    case npiFsdbHexStrVal:
      if ( !value.value.str )
        return 0;
      str = value.value.str;
      return 1;
    case npiFsdbSintVal:
      if ( sprintf( buf, "%-d", value.value.sint ) < 0 )
        return 0;
      str = buf;
      return 1;
    case npiFsdbUintVal:
      if ( sprintf( buf, "%-u", value.value.uint ) < 0 )
        return 0;
      str = buf;
      return 1;
    case npiFsdbRealVal:
      if ( sprintf( buf, "%-E", value.value.real ) < 0 )
        return 0;
      str = buf;
      return 1;
    case npiFsdbStringVal:
    case npiFsdbEnumStrVal:
      if ( !value.value.str )
        return 0;
      str = value.value.str;
      return 1;
    case npiFsdbSint64Val:
      if ( sprintf( buf, "%lld", value.value.sint64 ) < 0 )
        return 0;
      str = buf;
      return 1;
    case npiFsdbUint64Val:
      if ( sprintf( buf, "%llu", value.value.uint64 ) < 0 )
        return 0;
      str = buf;
      return 1;

    default:
      return 0;
  }
}


int fsdb_lib_vct_time_val( npiFsdbVctHandle vct,
                           const npiFsdbValType &format,
                           npiFsdbTime &vctTime/*O*/, 
                           string &val /*O*/) {
  if ( npi_fsdb_vct_time( vct, &vctTime ) == 0 )
    return 0;

  npiFsdbValue vctVal;
  vctVal.format = format;
  if ( npi_fsdb_vct_value( vct, &vctVal ) == 0 )
    return 0;
  
  fsdb_lib_val_to_str( vctVal, val );
  return 1;
}



  std::vector<std::pair<uint64_t, const char*>>*  fsdb_sig_value_between(npiFsdbFileHandle file_hdl, const char* sig_name, npiFsdbTime begin_time, npiFsdbTime end_time, npiFsdbValType format) {
    
    fsdbTimeValPairVec_t vc;

    if ( !file_hdl || !sig_name )
      return NULL;

    npiFsdbSigHandle sig = npi_fsdb_sig_by_name( file_hdl, sig_name, NULL );
    if ( !sig )
      return NULL;
    else {

      npiFsdbVctHandle vct = npi_fsdb_create_vct( sig );
      if ( !vct )
        return NULL;

      npiFsdbFileHandle file = npi_fsdb_sig_file( sig );
      npi_fsdb_add_to_sig_list( file_hdl, sig );
      npi_fsdb_load_vc_by_range( file_hdl, begin_time, end_time );

      npiFsdbTime vctTime;
      string val;
      if ( npi_fsdb_goto_time( vct, begin_time ) == 0 ) { // goto first VC
        npi_fsdb_release_vct( vct );
        return 0;
      }
      
      if ( fsdb_lib_vct_time_val( vct, format, vctTime, val ) == 0 ) {
        npi_fsdb_release_vct( vct );
        return 0;
      }
      vc.push_back( make_pair ( begin_time, val ) ); // first VC in this range
      
      while( npi_fsdb_goto_next( vct ) ) {
        if ( fsdb_lib_vct_time_val( vct, format, vctTime, val ) == 0 ) {
          npi_fsdb_release_vct( vct );
          return 0;
        }
        if ( vctTime > end_time )
          break;
        vc.push_back( make_pair( vctTime, val ) );
      }

      npi_fsdb_release_vct( vct );
    }

    // Now create a new vector to store pairs as (c_ulonglong, const char*)
    auto vec = new std::vector<std::pair<uint64_t, const char*>>();

    // Populate the new vector with data from fsdb_pairs
    for (const auto& pair : vc) {
        vec->emplace_back(pair.first, pair.second.c_str());
    }

    return vec;
  }

  void fsdb_end() {
    npi_end();
  }

  void fsdb_init(){
    vdb_init();
  }


  void vdb_init() {
#ifdef DUMMY_LIB
    return;
#else
    int argcv = 2;
    int& argc = argcv;

    char *args[3];

    // We need to mimic the regular argv format to success with NPI init
    args[0]= (char*)"./presifuzz\0";
    args[1]= (char*)"-q\0";
    args[2]=NULL;
    char **p_args=args;
    char**& argv = p_args;

    npi_init(argc, argv);
#endif
  }

  npiCovHandle vdb_cov_init(const char* vdb_file_path) {
#ifdef DUMMY_LIB
    return 0;
#else
    npiCovHandle db = npi_cov_open( vdb_file_path );

    return db;
#endif
  }

  void vdb_cov_end(npiCovHandle db) {
#ifdef DUMMY_LIB
    return;
#else
    npi_cov_close( db );
    npi_end();
#endif
  }

  void instance_map_size( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map)
  {
#ifdef DUMMY_LIB
    return;
#else
    npiCovHandle inst_iter = npi_cov_iter_start( npiCovInstance, scope );
    npiCovHandle inst = NULL;
    while ( (inst = npi_cov_iter_next( inst_iter )) )
    {
      std::string cov_full_name = npi_cov_get_str( npiCovFullName, inst); 
      if( cov_full_name.rfind(cov_map->filter, 0) == 0  ) {
        // printf( "%s\n", npi_cov_get_str( npiCovFullName, inst ));
        compute_size( inst, test, cov_map);
      }
      
      instance_map_size( inst, test, cov_map);
    }
    npi_cov_iter_stop( inst_iter );
#endif
  }

  void compute_size( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map)
  {
#ifdef DUMMY_LIB
    return;
#else
    npiCovHandle metric = npi_cov_handle( (npiCovObjType_e)cov_map->type, inst );
    npiCovHandle iter = npi_cov_iter_start( npiCovChild, metric );
    npiCovHandle block;
    while ( (block = npi_cov_iter_next( iter )) )
    {
      int covered =  npi_cov_get( npiCovCovered, block, test );
      if(covered < 0)
        covered = 0;

      int coverable = npi_cov_get( npiCovCoverable, block, NULL );

      cov_map->coverable = cov_map->coverable + coverable;
      cov_map->covered = cov_map->covered + covered;
    }
    npi_cov_iter_stop( iter );
#endif
  }

  void dump_instance_coverage( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map)
  {
#ifdef DUMMY_LIB
    return;
#else
    npiCovHandle inst_iter = npi_cov_iter_start( npiCovInstance, scope );
    npiCovHandle inst = NULL;
    while ( (inst = npi_cov_iter_next( inst_iter )) )
    {
      std::string cov_full_name = npi_cov_get_str( npiCovFullName, inst); 
      if( cov_full_name.rfind(cov_map->filter, 0) == 0  ) {
        // printf( "%s\n", npi_cov_get_str( npiCovFullName, inst ));
        compute_score( inst, test, cov_map);
      }
      
      dump_instance_coverage( inst, test, cov_map);
    }
    npi_cov_iter_stop( inst_iter );
#endif
  }

  void compute_score( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map)
  {
#ifdef DUMMY_LIB
    return;
#else
    npiCovHandle metric = npi_cov_handle( (npiCovObjType_e)cov_map->type, inst );
    npiCovHandle iter = npi_cov_iter_start( npiCovChild, metric );
    npiCovHandle block;
    while ( (block = npi_cov_iter_next( iter )) )
    {
      int covered =  npi_cov_get( npiCovCovered, block, test );
      if(covered < 0)
        covered = 0;

      int coverable = npi_cov_get( npiCovCoverable, block, NULL );

      cov_map->coverable = cov_map->coverable + coverable;
      cov_map->covered = cov_map->covered + covered;

      for(int i=0; i< covered; i++) {
          cov_map->map[cov_map->write_byte_index] |= ((uint32_t)1 << cov_map->write_bit_index);
          cov_map->write_bit_index += 1;      

          if( cov_map->write_bit_index == 32 ) {
            cov_map->write_byte_index += 1;
            cov_map->write_bit_index = 0;      
          } 
      }
      
      for(int i=0; i< coverable-covered; i++) {
          cov_map->map[cov_map->write_byte_index] &= ~((uint32_t)1 << cov_map->write_bit_index);
          cov_map->write_bit_index += 1;      

          if( cov_map->write_bit_index == 32 ) {
            cov_map->write_byte_index += 1;
            cov_map->write_bit_index = 0;      
          } 
      }
    }
    npi_cov_iter_stop( iter );
#endif
  }

  void update_cov_map(npiCovHandle db, uint32_t* map, unsigned map_size, unsigned coverage_type, char* filter) {
#ifdef DUMMY_LIB
    std::srand(std::time(nullptr));

    int start = std::rand() % map_size;
    int end = ((std::rand() % map_size) + start) % map_size;

    for(int i=start; i<end; i++) {
        unsigned cov_dist = std::rand() % 100;
        // non uniform distribution
        // P=0.2 increases coverage
        // P=0.7 do nothing
        if( cov_dist < 20) {
            map[i] = std::rand() % 0xFF;
       }
    }

    return;
#else
    CoverageMap cov_map;
    cov_map.map = map;
    cov_map.write_byte_index = 2;
    cov_map.write_bit_index = 0;
    cov_map.type = coverage_type;
    cov_map.size = map_size;
    cov_map.coverable = 0;
    cov_map.covered = 0;
    cov_map.filter = filter;

    // printf("COVERAGE: %d\n", coverage_type);
    // printf("FILTER: %s\n", filter);

    // Iterate test and merge test
    npiCovHandle test_iter = npi_cov_iter_start( npiCovTest, db );
    npiCovHandle test;
    npiCovHandle merged_test = NULL;
    while ( (test = npi_cov_iter_next( test_iter) ) )
    {
      if ( merged_test == NULL )
        merged_test = test;
      else
      {
        merged_test = npi_cov_merge_test( merged_test, test );
        if ( merged_test == NULL )
        {
          return;
        }
      }
    }
    npi_cov_iter_stop( test_iter );

    // Dump instance requested type score from top
    dump_instance_coverage((void*)db, merged_test, &cov_map);

    npi_cov_close( db );
    npi_end();

    // float score = 0.0;
    // if(cov_map.coverable != 0) {
      // score = (((float)cov_map.covered / (float)cov_map.coverable) * 100.0);
    // }
    // printf("score is %f\n", score);
    // printf("coverable is %d\n", cov_map.coverable);
    // printf("covered is %d\n", cov_map.covered);
    // assumption: float is 4bytes length, fits in u32
    // map[0] = (uint32_t)score;
    map[0] = cov_map.covered;
    map[1] = cov_map.coverable;
#endif
  }

#ifdef __cplusplus
}
#endif

size_t compute_map_size(npiCovHandle db, unsigned coverage_type, char* filter) {
#ifdef DUMMY_LIB
    return 1024;
#else
    CoverageMap cov_map;
    cov_map.map = NULL;
    cov_map.write_byte_index = 2;
    cov_map.write_bit_index = 0;
    cov_map.type = coverage_type;
    cov_map.size = 1024;
    cov_map.coverable = 0;
    cov_map.covered = 0;
    cov_map.filter = filter;

    // Iterate test and merge test
    npiCovHandle test_iter = npi_cov_iter_start( npiCovTest, db );
    npiCovHandle test;
    npiCovHandle merged_test = NULL;
    while ( (test = npi_cov_iter_next( test_iter) ) )
    {
      if ( merged_test == NULL )
        merged_test = test;
      else
      {
        merged_test = npi_cov_merge_test( merged_test, test );
        if ( merged_test == NULL )
        {
          return 0;
        }
      }
    }
    npi_cov_iter_stop( test_iter );

    // Dump instance requested type score from top
    instance_map_size((void*)db, merged_test, &cov_map);

    npi_cov_close( db );
    npi_end();

    return cov_map.coverable;
#endif 
}

#ifdef C_APP
#include "npi.h"
#include "npi_L1.h"
#include "npi_fsdb.h"

void traverse_hierarchy(npiFsdbHierHandle hier);
void traverse_signals(npiFsdbHierHandle hier);

void traverse_signals(npiFsdbHierHandle hier) {
    npiFsdbSigHandle sig;
    
    for (sig = npi_fsdb_hier_handle_sig_iter(hier); sig; sig = npi_fsdb_sig_handle_next(sig)) {
        const char* sig_name = npi_fsdb_sig_handle_full_name(sig);
        npiFsdbSigType sig_type = npi_fsdb_sig_handle_type(sig);
        printf("Signal: %s, Type: %d\n", sig_name, sig_type);
    }
}

void traverse_hierarchy(npiFsdbHierHandle hier) {
    const char* hier_name = npi_fsdb_hier_handle_full_name(hier);
    printf("Hierarchy: %s\n", hier_name);

    // Traverse signals within this hierarchy
    traverse_signals(hier);

    // Traverse sub-hierarchies
    npiFsdbHierHandle child;
    for (child = npi_fsdb_hier_handle_child_iter(hier); child; child = npi_fsdb_hier_handle_next(child)) {
        traverse_hierarchy(child);
    }
}

int main(int argc, char* argv[]) {
    if (argc != 2) {
        printf("Usage: %s <fsdb_file>\n", argv[0]);
        return 1;
    }

    const char* fsdb_file = argv[1];

    // Initialize the NPI and open the FSDB file
    if (!npi_init(NULL)) {
        printf("Failed to initialize NPI.\n");
        return 1;
    }

    if (!npi_fsdb_open(fsdb_file)) {
        printf("Failed to open FSDB file: %s\n", fsdb_file);
        npi_end();
        return 1;
    }

    // Get the top hierarchy handle
    npiFsdbHierHandle top = npi_fsdb_hier_handle_by_name("/");
    if (!top) {
        printf("Failed to get top hierarchy.\n");
        npi_fsdb_close();
        npi_end();
        return 1;
    }

    // Traverse the hierarchy starting from the top
    traverse_hierarchy(top);

    // Cleanup
    npi_fsdb_close();
    npi_end();

    return 0;
}

//int main(int argc, char** argv) {
//
//  vdb_init();
//
//  void* db = vdb_cov_init(argv[1]);
//  char* filter = "";
//
//  unsigned metric = atoi(argv[2]);
//
//  size_t size = compute_map_size(db, metric, filter);
//  printf("Map size is %d for metric %d", size, metric);
//
//  //uint32_t map[size] = {0};
//
//  //update_cov_map(db, (uint32_t*)&map, size, 5, filter);
//
//  //printf("[");
//  //unsigned i;
//  //for(i=0; i<size; i++) {
//  //  printf("%u ", map[i]);
//  //}
//  //printf("]");
//}
#endif
