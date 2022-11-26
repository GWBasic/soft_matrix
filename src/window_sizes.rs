use std::io::{Error, ErrorKind, Result};

// All of the optimial window sizes
// See https://docs.rs/rustfft/latest/rustfft/#avx-performance-tips
static WINDOW_SIZES: [usize; 61] = [6,12,18,24,36,48,54,72,96,108,144,162,192,216,288,324,384,432,486,576,648,768,864,972,1152,1296,1458,1536,1728,1944,2304,2592,2916,3072,3456,3888,4374,4608,5184,5832,6144,6912,7776,8748,9216,10368,11664,12288,13122,13824,15552,17496,18432,20736,23328,24576,26244,27648,31104,34992,36864];

/* 

C# program to generate window sizes

using System;
using System.Collections.Generic;

namespace FourierSizes
{
    class Program
    {
        const int HIGHEST_SAMPLING_RATE = 192000;
        const int LOWEST_FREQUENCY = 10;
        const int SMALLEST_WINDOW = HIGHEST_SAMPLING_RATE / LOWEST_FREQUENCY;
        const int LARGEST_WINDOW = SMALLEST_WINDOW * 2;

        static void Main(string[] args)
        {
            var windowSizes = new List<double>();
            for (var n = 1; n <= SMALLEST_WINDOW; n++)
                for (var m = 1; m <= SMALLEST_WINDOW; m++)
                {
                    var windowSize = Math.Pow(2, n) * Math.Pow(3, m);
                    if (windowSize <= LARGEST_WINDOW)
                    {
                        windowSizes.Add(windowSize);
                        Console.WriteLine($"2^{n} * 3^{m}: {windowSize}");
                    }
                }

            Console.WriteLine();

            windowSizes.Sort();
            var windowSizesCommaSeparated = string.Join(",", windowSizes);

            Console.WriteLine($"static WINDOW_SIZES: [u32; {windowSizes.Count}] = [{windowSizesCommaSeparated}];");
        }
    }
}


*/

pub fn get_ideal_window_size(min_window_size: usize) -> Result<usize> {
    for window_size in WINDOW_SIZES {
        if window_size >= min_window_size {
            return Ok(window_size);
        }
    }

    let error = format!("Can not find an ideal window size for {}", min_window_size);
    return Err(Error::new(ErrorKind::NotFound, error));
}