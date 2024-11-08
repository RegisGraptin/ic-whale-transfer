// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {ERC721} from "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";

contract WhaleNFT is ERC721 {
    
    uint256 private _nextTokenId;

    constructor() ERC721("WhaleNFT", "WNFT") {}
    
    function newWhale(address whaleAddress) public returns (uint256) {
        uint256 tokenId = _nextTokenId++;
        _mint(whaleAddress, tokenId);
        return tokenId;
    }
    
}
