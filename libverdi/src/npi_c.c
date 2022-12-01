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
 * 4. Traverse line metric
 * -------------------------------------------------------------------------------- */

#include "stdio.h"
#include "npi.h"
#include "npi_cov.h"

#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/stat.h>        /* For mode constants */
#include <fcntl.h>           /* For O_* constants */
#include <unistd.h>
#include <sys/types.h>
#include <string>
#include <random>


#ifdef __cplusplus
extern "C" {
#endif

  typedef struct {
    char* map;
    unsigned char write_bit_index;
    unsigned write_byte_index;
    unsigned type;
    unsigned size;
  }CoverageMap;
  
  void dump_instance_coverage( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map);
  npiCovHandle vdb_cov_init(const char* vdb_file_path);
  void vdb_cov_end(npiCovHandle db);
  float compute_score( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map);
  unsigned update_cov_map(npiCovHandle db, char* map, unsigned map_size, unsigned coverage_type);

  npiCovHandle vdb_cov_init(const char* vdb_file_path) {

    int argcv = 1;
    int& argc = argcv;

    char *args[2];

    // We need to mimic the regular argv format to success with NPI init
    args[0]= (char*)"/usr/bin/fuzzv_cov\0";
    args[1]=NULL;
    char **p_args=args; 
    char**& argv = p_args;
    
    npi_init(argc, argv);

    npiCovHandle db = npi_cov_open( vdb_file_path );
    if ( db == NULL )
    {
      return 0;
    }

    return db;
  }

  void vdb_cov_end(npiCovHandle db) {

    npi_cov_close( db );
    npi_end();
  }

  void dump_instance_coverage( npiCovHandle scope, npiCovHandle test, CoverageMap* cov_map)
  {
    npiCovHandle inst_iter = npi_cov_iter_start( npiCovInstance, scope );
    npiCovHandle inst = NULL;
    while ( (inst = npi_cov_iter_next( inst_iter )) )
    {
      compute_score( inst, test, cov_map);
      // printf( "%s: %f\n", npi_cov_get_str( npiCovFullName, inst ), score );

      dump_instance_coverage( inst, test, cov_map);
    }
    npi_cov_iter_stop( inst_iter );
  }

  float compute_score( npiCovHandle inst, npiCovHandle test, CoverageMap* cov_map)
  {
    int total_line = 0;
    int total_coverd = 0;

    npiCovHandle metric = npi_cov_handle( (npiCovObjType_e)cov_map->type, inst );
    npiCovHandle iter = npi_cov_iter_start( npiCovChild, metric );
    npiCovHandle block;
    while ( (block = npi_cov_iter_next( iter )) )
    {
      int covered =  npi_cov_get( npiCovCovered, block, test );
      total_line = total_line + npi_cov_get( npiCovCoverable, block, NULL );
      total_coverd = total_coverd + covered;
        
      cov_map->map[cov_map->write_byte_index++] = covered;
    }
    npi_cov_iter_stop( iter );

    if ( total_line == 0 )
      return 0.0;
    else
      return ((float) total_coverd / (float)total_line) * 100;
  }

  unsigned update_cov_map(npiCovHandle db, char* map, unsigned map_size, unsigned coverage_type) {

    CoverageMap cov_map;
    cov_map.map = map;
    cov_map.write_bit_index = 0;
    cov_map.write_byte_index = 0;
    cov_map.type = coverage_type;
    cov_map.size = map_size;

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
          return 1;
        }
      }
    }
    npi_cov_iter_stop( test_iter );

    // Dump instance requested type score from top
    dump_instance_coverage((void*)db, merged_test, &cov_map);

    npi_cov_close( db );
    npi_end();

    return 0;
  } 

#ifdef __cplusplus
}
#endif


#ifdef C_APP
int main(int argc, char** argv) {
  
  void* db = vdb_cov_init(argv[1]);

  unsigned size = 41678;
  // unsigned size = 41678;
  char map[size] = {0};

  update_cov_map(db, (char*) &map, size, 5);

  printf("[");
  unsigned i;
  for(i=0; i<size; i++) {
    printf("%d ", map[i]);
  }
  printf("]");
}
#endif
