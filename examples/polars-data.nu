let data_dir = ($env.FILE_PWD | path join data)
mkdir $data_dir
cd $data_dir

print "Downloading zip..."
http get "https://www.stats.govt.nz/assets/Uploads/New-Zealand-business-demography-statistics/New-Zealand-business-demography-statistics-At-February-2022/Download-data/CSV-data-load_data_metadata.zip"
  | save nz-stats.zip

print "Extracting zip..."
match $nu.os-info.name {
  "windows" => {tar -xf nz-stats.zip},
  _ => {unzip nz-stats.zip}
}

print "Done."
